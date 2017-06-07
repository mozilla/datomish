// Copyright 2016 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

#![allow(unused_imports)]

#[macro_use]
extern crate maplit;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate matches;

extern crate edn;

#[macro_use]
extern crate mentat_parser_utils;

mod parse;

pub use parse::{
    Error,
    ErrorKind,
    Result,
    ResultExt,
    parse_find_string,
};
