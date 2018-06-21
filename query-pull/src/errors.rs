// Copyright 2018 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use std; // To refer to std::result::Result.

use mentat_core::{
    Entid,
};

use failure::{
    Error,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Fail)]
pub enum PullError {
    #[fail(display = "attribute {:?} has no name", _0)]
    UnnamedAttribute(Entid),

    #[fail(display = ":db/id repeated")]
    RepeatedDbId,
}
