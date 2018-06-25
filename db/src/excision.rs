// Copyright 2018 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use std::collections::{
    BTreeSet,
    BTreeMap,
};

use rusqlite;

use mentat_core::{
    Attribute,
    Entid,
    HasSchema,
    Schema,
};

use entids;

use errors::{
    DbErrorKind,
    Result,
};

use internal_types::{
    AEVTrie,
    filter_aev_to_eav,
};

use schema::{
    SchemaBuilding,
};

use types::{
    PartitionMap,
};

/// Details about an excision:
/// - a target to excise (for now, an entid);
/// - a possibly empty set of attributes to excise (the empty set means all attributes, not no
///   attributes);
/// - and a possibly omitted transaction ID to limit the excision before.  (TODO: check whether
///   Datomic excises the last retraction before the first remaining assertion, and make our
///   behaviour agree.)
///
/// `:db/before` doesn't make sense globally, since in Mentat, monotonically increasing
/// transaction IDs don't guarantee monotonically increasing txInstant values.  Therefore, we
/// accept only `:db/beforeT` and allow consumers to turn `:db/before` timestamps into
/// transaction IDs in whatever way they see fit.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) struct Excision {
    pub(crate) target: Entid,
    pub(crate) attrs: Option<BTreeSet<Entid>>,
    pub(crate) before_tx: Option<Entid>,
}

/// Map from `entid` to excision details.  `entid` is not the excision `target`!
pub(crate) type ExcisionMap = BTreeMap<Entid, Excision>;

/// Extract excisions from the given transacted datoms.
pub(crate) fn excisions<'schema>(partition_map: &'schema PartitionMap, schema: &'schema Schema, aev_trie: &AEVTrie<'schema>) -> Result<Option<ExcisionMap>> {
    let pair = |a: Entid| -> Result<(Entid, &'schema Attribute)> {
        schema.require_attribute_for_entid(a).map(|attribute| (a, attribute))
    };

    if aev_trie.contains_key(&pair(entids::DB_EXCISE_BEFORE)?) {
        bail!(DbErrorKind::BadExcision(":db.excise/before".into())); // TODO: more details.
    }

    // TODO: Don't allow anything more than excisions in the excising transaction, except
    // additional facts about the (transaction-tx).
    let eav_trie = filter_aev_to_eav(aev_trie, |&(a, _)|
                                     a == entids::DB_EXCISE ||
                                     a == entids::DB_EXCISE_ATTRS ||
                                     a == entids::DB_EXCISE_BEFORE_T);

    let mut excisions = ExcisionMap::default();

    for (&e, avs) in eav_trie.iter() {
        for (&(_a, _attribute), ars) in avs {
            if !ars.retract.is_empty() {
                bail!(DbErrorKind::BadExcision("retraction".into())); // TODO: more details.
            }
        }

        let target = avs.get(&pair(entids::DB_EXCISE)?)
            .and_then(|ars| ars.add.iter().next().cloned())
            .and_then(|v| v.into_entid())
            .ok_or_else(|| DbErrorKind::BadExcision("no :db/excise".into()))?; // TODO: more details.

        if schema.get_ident(target).is_some() {
            bail!(DbErrorKind::BadExcision("cannot mutate schema".into())); // TODO: more details.
        }

        let partition = partition_map.partition_for_entid(target)
            .ok_or_else(|| DbErrorKind::BadExcision("target has no partition".into()))?; // TODO: more details.
        // Right now, Mentat only supports `:db.part/{db,user,tx}`, and tests hack in `:db.part/fake`.
        if partition == ":db.part/db" || partition == ":db.part/tx" {
            bail!(DbErrorKind::BadExcision(format!("cannot target entity in partition {}", partition).into())); // TODO: more details.
        }

        let before_tx = avs.get(&pair(entids::DB_EXCISE_BEFORE_T)?)
            .and_then(|ars| ars.add.iter().next().cloned())
            .and_then(|v| v.into_entid());

        let attrs = avs.get(&pair(entids::DB_EXCISE_ATTRS)?)
            .map(|ars| ars.add.clone().into_iter().filter_map(|v| v.into_entid()).collect());

        let excision = Excision {
            target,
            attrs,
            before_tx,
        };

        excisions.insert(e, excision);
    }

    if excisions.is_empty() {
        Ok(None)
    } else {
        Ok(Some(excisions))
    }
}

pub(crate) fn enqueue_pending_excisions(conn: &rusqlite::Connection, schema: &Schema, tx_id: Entid, excisions: ExcisionMap) -> Result<()> {
    // excisions.into_iter().map(|(entid, excision)| enqueue_pending_excision(self, entid, excision)).collect().and(Ok(()))

    // if !excisions.is_none() {
    //     bail!(DbError::NotYetImplemented(format!("Excision not yet implemented: {:?}", excisions)));
    // }

    let mut stmt1: rusqlite::Statement = conn.prepare("INSERT INTO excisions VALUES (?, ?, ?, ?)")?;
    let mut stmt2: rusqlite::Statement = conn.prepare("INSERT INTO excision_attrs VALUES (?, ?)")?;

    for (entid, excision) in excisions {
        stmt1.execute(&[&entid, &excision.target, &excision.before_tx, &excision.before_tx.unwrap_or(tx_id)])?; // XXX
        if let Some(attrs) = excision.attrs {
            // println!("attrs {:?}", attrs);
            for attr in attrs {
                stmt2.execute(&[&entid, &attr])?;
            }
        }
    }

    // TODO: filter by attrs.
    let mut stmt: rusqlite::Statement = conn.prepare(format!("WITH ids AS (SELECT d.rowid FROM datoms AS d, excisions AS e WHERE e.status > 0 AND (e.target IS d.e OR (e.target IS d.v AND d.a IS NOT {}))) DELETE FROM datoms WHERE rowid IN ids", entids::DB_EXCISE).as_ref())?;

    stmt.execute(&[])?;

    Ok(())
}

pub(crate) fn pending_excisions(conn: &rusqlite::Connection, partition_map: &PartitionMap, schema: &Schema) -> Result<ExcisionMap> {
    let mut stmt1: rusqlite::Statement = conn.prepare("SELECT e, target, before_tx, status FROM excisions WHERE status > 0 ORDER BY e")?;
    let mut stmt2: rusqlite::Statement = conn.prepare("SELECT a FROM excision_attrs WHERE e IS ?")?;

    let m: Result<ExcisionMap> = stmt1.query_and_then(&[], |row| {
        let e: Entid = row.get_checked(0)?;
        let target: Entid = row.get_checked(1)?;
        let before_tx: Option<Entid> = row.get_checked(2)?;

        let attrs: Result<BTreeSet<Entid>> = stmt2.query_and_then(&[&e], |row| {
            let a: Entid = row.get_checked(0)?;
            Ok(a)
        })?.collect();
        let attrs = attrs.map(|attrs| {
            if attrs.is_empty() {
                None
            } else {
                Some(attrs)
            }
        })?;

        let excision = Excision {
            target,
            before_tx,
            attrs,
        };

        Ok((e, excision))
    })?.collect();

    m

    // let aev_trie = read_materialized_transaction_aev_trie(&conn, schema, "excisions")?;

    // excisions(&partition_map, &schema, &aev_trie).map(|o| o.unwrap_or_default())
}

pub(crate) fn ensure_no_pending_excisions(conn: &rusqlite::Connection) -> Result<()> {
    // let pending = pending_excisions(self)?;

        // WITH ids AS (SELECT rid
        //              FROM temp.search_results
        //              WHERE rid IS NOT NULL AND
        //                    ((added0 IS 0) OR
        //                     (added0 IS 1 AND search_type IS ':db.cardinality/one' AND v0 IS NOT v)))
        // DELETE FROM datoms WHERE rowid IN ids"#;

    // TODO: filter by attrs.
    let mut stmt: rusqlite::Statement = conn.prepare(format!("WITH ids AS (SELECT t.rowid FROM transactions AS t, excisions AS e WHERE e.status > 0 AND t.tx <= e.status AND (e.target IS t.e OR (e.target IS t.v AND t.a IS NOT {}))) DELETE FROM transactions WHERE rowid IN ids", entids::DB_EXCISE).as_ref())?;

    stmt.execute(&[])?;

    let mut stmt: rusqlite::Statement = conn.prepare("UPDATE excisions SET status = 0")?;

    stmt.execute(&[])?;

    // let relevant_tx_ids: Result<Vec<Entid>> = stmt.query_and_then(&[], |row| {
    //     let e: Entid = row.get_checked(0)?;
    //     let target:
    //     Ok(tx)
    // })?.collect();

    // println!("relevant_tx_ids: {:?}", relevant_tx_ids?);

    // let mut stmt: rusqlite::Statement = conn.prepare(format!("DELETE FROM transactions AS t WHERE t.tx = ? AND (t.e = ? OR (t.v = ? AND t.a != {}))", entids::DB_EXCISE).as_str());

    // for tx_id in relevant_tx_ids {

    // }

    Ok(())
}

// fn enqueue_pending_excision(conn: &rusqlite::Connection, excision: Excision) -> Result<()> {
//     Ok(())
// }
