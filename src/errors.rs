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

use std; // To refer to std::result::Result.

use std::collections::BTreeSet;

use rusqlite;
use uuid;

use edn;

use mentat_core::{
    Attribute,
    ValueType,
};

use mentat_db;
use mentat_query;
use mentat_query_algebrizer;
use mentat_query_projector;
use mentat_query_pull;
use mentat_sql;

#[cfg(feature = "syncable")]
use mentat_tolstoy;

pub type Result<T> = std::result::Result<T, MentatError>;

#[macro_export]
macro_rules! bail {
    ($e:expr) => (
        return Err($e.into());
    )
}

#[derive(Debug, Fail)]
pub enum MentatError {
    #[fail(display = "bad uuid {}", _0)]
    BadUuid(String),

    #[fail(display = "path {} already exists", _0)]
    PathAlreadyExists(String),

    #[fail(display = "variables {:?} unbound at query execution time", _0)]
    UnboundVariables(BTreeSet<String>),

    #[fail(display = "invalid argument name: '{}'", _0)]
    InvalidArgumentName(String),

    #[fail(display = "unknown attribute: '{}'", _0)]
    UnknownAttribute(String),

    #[fail(display = "invalid vocabulary version")]
    InvalidVocabularyVersion,

    #[fail(display = "vocabulary {}/{} already has attribute {}, and the requested definition differs", _0, _1, _2)]
    ConflictingAttributeDefinitions(String, ::vocabulary::Version, String, Attribute, Attribute),

    #[fail(display = "existing vocabulary {} too new: wanted {}, got {}", _0, _1, _2)]
    ExistingVocabularyTooNew(String, ::vocabulary::Version, ::vocabulary::Version),

    #[fail(display = "core schema: wanted {}, got {:?}", _0, _1)]
    UnexpectedCoreSchema(::vocabulary::Version, Option<::vocabulary::Version>),

    #[fail(display = "Lost the transact() race!")]
    UnexpectedLostTransactRace,

    #[fail(display = "missing core attribute {}", _0)]
    MissingCoreVocabulary(mentat_query::Keyword),

    #[fail(display = "schema changed since query was prepared")]
    PreparedQuerySchemaMismatch,

    #[fail(display = "provided value of type {} doesn't match attribute value type {}", _0, _1)]
    ValueTypeMismatch(ValueType, ValueType),

    /// We're just not done yet.  Message that the feature is recognized but not yet
    /// implemented.
    #[fail(display = "not yet implemented: {}", _0)]
    NotYetImplemented(String),

    #[fail(display = "{}", _0)]
    IoError(#[cause] std::io::Error),

    // It would be better to capture the underlying `rusqlite::Error`, but that type doesn't
    // implement many useful traits, including `Clone`, `Eq`, and `PartialEq`.
    #[fail(display = "SQL error: _0")]
    RusqliteError(String),

    #[fail(display = "{}", _0)]
    EdnParseError(#[cause] edn::ParseError),

    #[fail(display = "{}", _0)]
    DbError(#[cause] mentat_db::DbError),

    #[fail(display = "{}", _0)]
    AlgebrizerError(#[cause] mentat_query_algebrizer::AlgebrizerError),

    #[fail(display = "{}", _0)]
    ProjectorError(#[cause] mentat_query_projector::ProjectorError),

    #[fail(display = "{}", _0)]
    PullError(#[cause] mentat_query_pull::PullError),

    #[fail(display = "{}", _0)]
    UuidError(#[cause] uuid::ParseError),

    #[fail(display = "{}", _0)]
    SQLError(#[cause] mentat_sql::SQLError),

    #[cfg(feature = "syncable")]
    #[fail(display = "{}", _0)]
    TolstoyError(#[cause] mentat_tolstoy::TolstoyError),
}

impl From<std::io::Error> for MentatError {
    fn from(error: std::io::Error) -> MentatError {
        MentatError::IoError(error)
    }
}

impl From<rusqlite::Error> for MentatError {
    fn from(error: rusqlite::Error) -> MentatError {
        MentatError::RusqliteError(error.to_string())
    }
}

impl From<edn::ParseError> for MentatError {
    fn from(error: edn::ParseError) -> MentatError {
        MentatError::EdnParseError(error)
    }
}

impl From<mentat_db::DbError> for MentatError {
    fn from(error: mentat_db::DbError) -> MentatError {
        MentatError::DbError(error)
    }
}

impl From<mentat_query_algebrizer::AlgebrizerError> for MentatError {
    fn from(error: mentat_query_algebrizer::AlgebrizerError) -> MentatError {
        MentatError::AlgebrizerError(error)
    }
}

impl From<mentat_query_projector::ProjectorError> for MentatError {
    fn from(error: mentat_query_projector::ProjectorError) -> MentatError {
        MentatError::ProjectorError(error)
    }
}

impl From<mentat_query_pull::PullError> for MentatError {
    fn from(error: mentat_query_pull::PullError) -> MentatError {
        MentatError::PullError(error)
    }
}

impl From<mentat_sql::SQLError> for MentatError {
    fn from(error: mentat_sql::SQLError) -> MentatError {
        MentatError::SQLError(error)
    }
}

#[cfg(feature = "syncable")]
impl From<mentat_tolstoy::TolstoyError> for MentatError {
    fn from(error: mentat_tolstoy::TolstoyError) -> MentatError {
        MentatError::TolstoyError(error)
    }
}

impl From<uuid::ParseError> for MentatError {
    fn from(error: uuid::ParseError) -> MentatError {
        MentatError::UuidError(error)
    }
}
