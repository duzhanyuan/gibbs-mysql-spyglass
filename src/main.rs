// Gibbs MySQL Spyglass
// Copyright (C) 2016 AgilData
//
// This file is part of Gibbs MySQL Spyglass.
//
// Gibbs MySQL Spyglass is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Gibbs MySQL Spyglass is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Gibbs MySQL Spyglass.  If not, see <http://www.gnu.org/licenses/>.

#![feature(plugin)]
#![plugin(regex_macros)]

#[macro_use]
extern crate log;
extern crate log4rs;

extern crate hyper;

extern crate time;

extern crate regex;

use std::thread;

mod util;
use util::{COpts, TMP_FILE};
use std::net::{IpAddr, Ipv4Addr};
use std::fmt::Display;

mod capture;
use capture::client::schema;
use capture::sniffer::sniff;

mod comm;
use comm::upload;

use std::cell::RefCell;
use std::fs::{self, File, OpenOptions};
use std::io;

thread_local!(static OUT: RefCell<File> =
    RefCell::new(OpenOptions::new().read(true).append(true).create(true).open(TMP_FILE).unwrap())
);

#[derive(Clone, Debug, Eq, PartialEq)]
enum CLIState {
    Welcome,
    AskKey,
    ChkKey,
    AskHost,
    ChkHost,
    AskPort,
    ChkPort,
    AskUser,
    ChkUser,
    AskPass,
    ChkPass,
    AskDb,
    ChkDb,
    AskIface,
    ChkIface,
    AskStart,
    ChkStart,
    AskStop,
    ChkStop,
    AskSend,
    ChkSend,
    Quit,
}

use CLIState::*;

fn again(msg: &str, dflt: &Display) {
    println!("{}, please try again {} ", msg, dflt);
}

fn cli_act(lst: CLIState, inp: &str, opt: &mut COpts) -> CLIState { match lst {
    Welcome => {
        println!("Welcome to Gibbs' Spyglass MySQL Traffic Capture Tool.");
        cli_act(AskKey, "", opt)
    },
    AskKey => {
        println!("What is your API Key (get one at https://gibbs.agildata.com/)? [{}] ", opt.key);
        ChkKey
    },
    ChkKey => {
        if inp.len() != 40 {
            again("Key must be 40 hex characters long", &opt.key);
            ChkKey
        // } else if { check for all hex characters here
        } else {
            opt.key = inp.to_owned();
            cli_act(AskHost, "", opt)
        }
    },
    AskHost => {
        println!("Great! Let's set up your MySQL connection. What's your MySQL host? [{}] ", opt.host);
        ChkHost
    },
    ChkHost => {
        if inp.len() > 0 {
            match inp.parse::<IpAddr>() {
                Ok(h) => {
                    opt.host = h;
                    cli_act(AskPort, "", opt)
                },
                Err(e) => {
                    again(&e.to_string(), &opt.host);
                    lst
                },
            }
        } else { cli_act(AskPort, "", opt) }
     },
    AskPort => {
        println!("And your MySQL port? [{}] ", opt.port);
        ChkPort
    },
    ChkPort => {
        if inp.len() > 0 {
            match u16::from_str_radix(&inp, 10) {
                Ok(p) => {
                    opt.port = p;
                    cli_act(AskUser, "", opt)
                },
                Err(e) => {
                    again(&e.to_string(), &opt.port);
                    lst
                },
            }
        } else { cli_act(AskUser, "", opt) }
    },
    AskUser => {
        println!("And your MySQL username? [{}] ", opt.user);
        ChkUser
    },
    ChkUser => {
        if inp.len() > 0 { opt.user = inp.to_owned(); }
        cli_act(AskPass, "", opt)
    },
    AskPass => {
        println!("And your MySQL password? [] ");
        ChkPass
    },
    ChkPass => {
        if inp.len() > 0 { opt.pass = inp.to_owned(); }
        cli_act(AskDb, "", opt)
    },
    AskDb => {
        println!("And the MySQL database to analyze? [{}] ", opt.db);
        ChkDb
    },
    ChkDb => {
        if inp.len() > 0 { opt.db = inp.to_owned(); }
        print!("Querying schema");
        schema(opt.clone());
        println!("\nSchema done.");
        cli_act(AskIface, "", opt)
    },
    AskIface => {
        println!("And finally, your network interface carrying MySQL traffic? (eth0, en0, ...) [{}] ", opt.iface);
        ChkIface
    },
    ChkIface => {
        if inp.len() > 0 { opt.iface = inp.to_owned(); }
        cli_act(AskStart, "", opt)
    },
    AskStart => {
        println!("Great! We're all set. Press enter to start data capture.");
        ChkStart
    },
    ChkStart => {
        cli_act(AskStop, "", opt)
    },
    AskStop => {
        print!("Starting capture, press enter to stop.");
        let sniff_opt = opt.clone();
        let _= thread::spawn(|| {
            sniff(sniff_opt);
        });
        ChkStop
    },
    ChkStop => {
        println!("\nData capture stopped. We found XX queries, totaling YYMB of data.");
        cli_act(AskSend, "", opt)
    },
    AskSend => {
        println!("Would you like to send this to Gibbs now? [y] ");
        ChkSend
    },
    ChkSend => {
        if inp.len() == 0 || inp.to_string().to_uppercase() == "Y" {
            print!("Sending...");
            upload(opt.clone());
            println!(".done! Press enter to complete.");
        }
        Quit
    },
    Quit => panic!("should never be processing a Quit state"),
} }

fn main() {
    let _ = fs::remove_file(TMP_FILE);
    let _ = log4rs::init_file("spyglass.toml", Default::default());

    let mut st: CLIState = Welcome;
    let mut inp = String::new();
    let mut opt = COpts {
        key: "".to_string(),
        host: IpAddr::V4(Ipv4Addr::new(0,0,0,0)),
        port: 3306,
        user: "root".to_string(),
        pass: "".to_string(),
        db: "".to_string(),
        iface: "".to_string(),
        tx: None,
    };

    while st != Quit {
        st = cli_act(st, &inp, &mut opt);
        inp.clear();
        match io::stdin().read_line(&mut inp) {
            Ok(_) => { inp.pop(); },
            Err(e) => again(&e.to_string(), &""),
        }
    }

}
