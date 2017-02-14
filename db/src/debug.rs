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

/// Low-level functions for testing.

use std::collections::{BTreeSet};
use std::io::{Write};

use itertools::Itertools;
use rusqlite;
use rusqlite::types::{ToSql};
use tabwriter::TabWriter;

use ::{to_namespaced_keyword};
use bootstrap;
use edn;
use edn::symbols;
use entids;
use mentat_core::TypedValue;
use mentat_tx::entities::{Entid};
use db::TypedSQLValue;
use types::DB;
use errors::Result;

/// Represents a *datom* (assertion) in the store.
#[derive(Clone,Debug,Eq,Hash,Ord,PartialOrd,PartialEq)]
pub struct Datom {
    // TODO: generalize this.
    e: Entid,
    a: Entid,
    v: edn::Value,
    tx: i64,
    added: Option<bool>,
}

/// Represents a set of datoms (assertions) in the store.
pub struct Datoms(pub BTreeSet<Datom>);

/// Represents an ordered sequence of transactions in the store.
pub struct Transactions(pub Vec<Datoms>);

fn label_tx_id(tx: i64) -> edn::Value {
    edn::Value::PlainSymbol(symbols::PlainSymbol::new(format!("?tx{}", tx - bootstrap::TX0)))
}

fn label_tx_instant(tx: i64) -> edn::Value {
    edn::Value::PlainSymbol(symbols::PlainSymbol::new(format!("?ms{}", tx - bootstrap::TX0)))
}

impl Datom {
    pub fn into_edn<T, U>(&self, tx_id: T, tx_instant: &U) -> edn::Value
        where T: Fn(i64) -> edn::Value, U: Fn(i64) -> edn::Value {
        let f = |entid: &Entid| -> edn::Value {
            match *entid {
                Entid::Entid(ref y) => edn::Value::Integer(y.clone()),
                Entid::Ident(ref y) => edn::Value::NamespacedKeyword(y.clone()),
            }
        };

        // Rewrite [E :db/txInstant V] to [?txN :db/txInstant ?t0].
        let mut v = if self.a == Entid::Entid(entids::DB_TX_INSTANT) || self.a == Entid::Ident(to_namespaced_keyword(":db/txInstant").unwrap()) {
            vec![tx_id(self.tx),
                 f(&self.a),
                 tx_instant(self.tx)]
        } else {
            vec![f(&self.e), f(&self.a), self.v.clone()]
        };
        if let Some(added) = self.added {
            v.push(tx_id(self.tx));
            v.push(edn::Value::Boolean(added));
        }

        edn::Value::Vector(v)
    }
}

impl Datoms {
    pub fn into_edn_raw<T, U>(&self, tx_id: &T, tx_instant: &U) -> edn::Value
        where T: Fn(i64) -> edn::Value, U: Fn(i64) -> edn::Value {
        edn::Value::Set((&self.0).into_iter().map(|x| x.into_edn(tx_id, tx_instant)).collect())
    }

    pub fn into_edn(&self) -> edn::Value {
        self.into_edn_raw(&label_tx_id, &label_tx_instant)
    }
}

impl Transactions {
    pub fn into_edn_raw<T, U>(&self, tx_id: &T, tx_instant: &U) -> edn::Value
        where T: Fn(i64) -> edn::Value, U: Fn(i64) -> edn::Value {
        edn::Value::Vector((&self.0).into_iter().map(|x| x.into_edn_raw(tx_id, tx_instant)).collect())
    }

    pub fn into_edn(&self) -> edn::Value {
        self.into_edn_raw(&label_tx_id, &label_tx_instant)
    }
}

/// Convert a numeric entid to an ident `Entid` if possible, otherwise a numeric `Entid`.
fn to_entid(db: &DB, entid: i64) -> Entid {
    db.schema.get_ident(entid).and_then(|ident| to_namespaced_keyword(&ident)).map_or(Entid::Entid(entid), Entid::Ident)
}

/// Return the set of datoms in the store, ordered by (e, a, v, tx), but not including any datoms of
/// the form [... :db/txInstant ...].
pub fn datoms(conn: &rusqlite::Connection, db: &DB) -> Result<Datoms> {
    datoms_after(conn, db, bootstrap::TX0 - 1)
}

/// Return the set of datoms in the store with transaction ID strictly greater than the given `tx`,
/// ordered by (e, a, v, tx).
///
/// The datom set returned does not include any datoms of the form [... :db/txInstant ...].
pub fn datoms_after(conn: &rusqlite::Connection, db: &DB, tx: i64) -> Result<Datoms> {
    let mut stmt: rusqlite::Statement = conn.prepare("SELECT e, a, v, value_type_tag, tx FROM datoms WHERE tx > ? ORDER BY e ASC, a ASC, v ASC, tx ASC")?;

    let r: Result<Vec<_>> = stmt.query_and_then(&[&tx], |row| {
        let e: i64 = row.get_checked(0)?;
        let a: i64 = row.get_checked(1)?;

        if a == entids::DB_TX_INSTANT {
            return Ok(None);
        }

        let v: rusqlite::types::Value = row.get_checked(2)?;
        let value_type_tag: i32 = row.get_checked(3)?;

        let typed_value = TypedValue::from_sql_value_pair(v, value_type_tag)?;
        let (value, _) = typed_value.to_edn_value_pair();

        let tx: i64 = row.get_checked(4)?;

        Ok(Some(Datom {
            e: to_entid(db, e),
            a: to_entid(db, a),
            v: value,
            tx: tx,
            added: None,
        }))
    })?.collect();

    Ok(Datoms(r?.into_iter().filter_map(|x| x).collect()))
}

/// Return the sequence of transactions in the store with transaction ID strictly greater than the
/// given `tx`, ordered by (tx, e, a, v).
///
/// Each transaction returned includes the [:db/tx :db/txInstant ...] datom.
pub fn transactions_after(conn: &rusqlite::Connection, db: &DB, tx: i64) -> Result<Transactions> {
    let mut stmt: rusqlite::Statement = conn.prepare("SELECT e, a, v, value_type_tag, tx, added FROM transactions WHERE tx > ? ORDER BY tx ASC, e ASC, a ASC, v ASC, added ASC")?;

    let r: Result<Vec<_>> = stmt.query_and_then(&[&tx], |row| {
        let e: i64 = row.get_checked(0)?;
        let a: i64 = row.get_checked(1)?;

        let v: rusqlite::types::Value = row.get_checked(2)?;
        let value_type_tag: i32 = row.get_checked(3)?;

        let typed_value = TypedValue::from_sql_value_pair(v, value_type_tag)?;
        let (value, _) = typed_value.to_edn_value_pair();

        let tx: i64 = row.get_checked(4)?;
        let added: bool = row.get_checked(5)?;

        Ok(Datom {
            e: to_entid(db, e),
            a: to_entid(db, a),
            v: value,
            tx: tx,
            added: Some(added),
        })
    })?.collect();

    // Group by tx.
    let r: Vec<Datoms> = r?.into_iter().group_by(|x| x.tx).into_iter().map(|(_key, group)| Datoms(group.collect())).collect();
    Ok(Transactions(r))
}

/// Execute the given `sql` query with the given `params` and format the results as a
/// tab-and-newline formatted string suitable for debug printing.
///
/// The query is printed followed by a newline, then the returned columns followed by a newline, and
/// then the data rows and columns.  All columns are aligned.
pub fn dump_sql_query(conn: &rusqlite::Connection, sql: &str, params: &[&ToSql]) -> Result<String> {
    let mut stmt: rusqlite::Statement = conn.prepare(sql)?;

    let mut tw = TabWriter::new(Vec::new()).padding(2);
    write!(&mut tw, "{}\n", sql).unwrap();

    for column_name in stmt.column_names() {
        write!(&mut tw, "{}\t", column_name).unwrap();
    }
    write!(&mut tw, "\n").unwrap();

    let r: Result<Vec<_>> = stmt.query_and_then(params, |row| {
        for i in 0..row.column_count() {
            let value: rusqlite::types::Value = row.get_checked(i)?;
            write!(&mut tw, "{:?}\t", value).unwrap();
        }
        write!(&mut tw, "\n").unwrap();
        Ok(())
    })?.collect();
    r?;

    let dump = String::from_utf8(tw.into_inner().unwrap()).unwrap();
    Ok(dump)
}
