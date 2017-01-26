// Copyright 2016 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

#[macro_use] extern crate log;
extern crate env_logger;
extern crate clap;
#[macro_use] extern crate nickel;

use nickel::{Nickel, HttpRouter};

use clap::{App, Arg, SubCommand, AppSettings};

use std::env;
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

        init_logger(debug).unwrap();
        info!("Starting Mentat ({})", env!("CARGO_PKG_VERSION"));

        let port = u16::from_str(matches.value_of("port").unwrap()).expect("Port must be an integer");
        info!("This doesn't do anything yet, but it will eventually serve up the following database: {}",
              matches.value_of("database").unwrap());

        let mut server = Nickel::new();
        server.get("/", middleware!("This doesn't do anything yet"));
        server.listen(("127.0.0.1", port)).expect("Failed to launch server");
    }
}

fn init_logger(debug_mode: bool) -> Result<(), log::SetLoggerError> {
    let mut builder = env_logger::LogBuilder::new();

    let env_vars = env::var("MENTAT_LOG").or(env::var("RUST_LOG"));
    if let Ok(s) = env_vars {
        builder.parse(&s);
    } else {
        let log_level = if debug_mode {log::LogLevelFilter::Debug} else {log::LogLevelFilter::Warn};
        builder.filter(None, log_level);
    }
    builder.init()
}
