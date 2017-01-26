// Copyright 2016 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

extern crate clap;
#[macro_use] extern crate nickel;

use nickel::{Nickel, HttpRouter};

#[macro_use]
extern crate slog;
#[macro_use]
extern crate slog_scope;
extern crate slog_term;

extern crate mentat;

use clap::{App, Arg, SubCommand, AppSettings};
use slog::DrainExt;

use std::u16;
use std::str::FromStr;

fn main() {
    let app = App::new("Mentat").setting(AppSettings::ArgRequiredElseHelp);
    let matches = app.subcommand(SubCommand::with_name("serve")
            .about("Starts a server")
            .arg(Arg::with_name("debug")
                .long("debug")
                .help("Print debugging info"))
            .arg(Arg::with_name("database")
                .short("d")
                .long("database")
                .value_name("FILE")
                .help("Path to the Mentat database to serve")
                .default_value("")
                .takes_value(true))
            .arg(Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("INTEGER")
                .help("Port to serve from, i.e. `localhost:PORT`")
                .default_value("3333")
                .takes_value(true)))
        .get_matches();
    if let Some(ref matches) = matches.subcommand_matches("serve") {
        let debug = matches.is_present("debug");
        let port = u16::from_str(matches.value_of("port").unwrap()).expect("Port must be an integer");
        if debug {
            println!("This doesn't do anything yet, but it will eventually serve up the following database: {} \
                      on port: {}.",
                     matches.value_of("database").unwrap(),
                     matches.value_of("port").unwrap());
        }

        // Set up logging.
        let log_level = if debug {
            slog::Level::Debug
        } else {
            slog::Level::Warning
        };
        let term_logger = slog_term::streamer().build().fuse();
        let log = slog::Logger::root(slog::LevelFilter::new(term_logger, log_level),
                                     o!("version" => env!("CARGO_PKG_VERSION")));
        slog_scope::set_global_logger(log);

        info!("Serving database"; "database" => matches.value_of("database").unwrap(),
                                  "port" => port,
                                  "debug mode" => debug);

        error!("Calling a function: {}", mentat::get_name());

        let mut server = Nickel::new();
        server.get("/", middleware!("This doesn't do anything yet"));
        server.listen(("127.0.0.1", port)).expect("Failed to launch server");
    }
}
