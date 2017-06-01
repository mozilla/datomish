// Copyright 2016 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

#![allow(dead_code)]

use rusqlite;

use mentat::errors as mentat;
use edn;

error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        EdnParseError(edn::ParseError);
        Rusqlite(rusqlite::Error);
    }

    links {
        MentatError(mentat::Error, mentat::ErrorKind);
    }

    errors {
        CommandParse(message: String) {
            description("An error occured parsing the entered command")
            display("{}", message)
        }

        FileError(filename: String, message: String) {
            description("An error occured while reading file")
            display("Unable to open file {}: {}", filename, message)
        }
    }
}
