// Copyright 2016 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use std::collections::BTreeMap;

use rusqlite;

use mentat_core::{
    Entid,
    TypedValue,
};

use mentat_db::cache::{
    AttributeValueProvider,
    Cacheable,
    EagerCache,
    CacheMap,
};

use errors::{
    Result,
};

pub enum CacheAction {
    Register,
    Deregister,
}

#[derive(Clone)]
pub struct AttributeCacher {
    a_e_vs_cache: BTreeMap<Entid, EagerCache<Entid, Vec<TypedValue>, AttributeValueProvider>>,   // values keyed by attribute
}

impl AttributeCacher {

    pub fn new() -> Self {
        AttributeCacher {
            a_e_vs_cache: BTreeMap::new(),
        }
    }

    pub fn register_attribute<'sqlite>(&mut self, sqlite: &'sqlite rusqlite::Connection, attribute: Entid) -> Result<()> {
        let value_provider = AttributeValueProvider{ attribute: attribute };
        let mut cacher = EagerCache::new(value_provider);
        cacher.cache_values(sqlite)?;
        self.a_e_vs_cache.insert(attribute, cacher);
        Ok(())
    }

    pub fn deregister_attribute(&mut self, attribute: &Entid) -> Option<CacheMap<Entid, Vec<TypedValue>>> {
        self.a_e_vs_cache.remove(&attribute).map(|m| m.cache)
    }

    pub fn get(&self, attribute: &Entid) -> Option<&CacheMap<Entid, Vec<TypedValue>>> {
        self.a_e_vs_cache.get( &attribute ).map(|m| &m.cache)
    }

    pub fn get_values_for_entid(&self, attribute: &Entid, entid: &Entid) -> Option<&Vec<TypedValue>> {
        self.a_e_vs_cache.get(&attribute).and_then(|c| c.get(&entid))
    }

    pub fn get_value_for_entid(&self, attribute: &Entid, entid: &Entid) -> Option<&TypedValue> {
        self.get_values_for_entid(attribute, entid).and_then(|c| c.first())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use mentat_core::{
        HasSchema,
        KnownEntid,
    };
    use mentat_db::db;
    use mentat_db::types::TypedValue;

    use conn::Conn;

    fn populate_db() -> (Conn, rusqlite::Connection) {
        let mut sqlite = db::new_connection("").unwrap();
        let mut conn = Conn::connect(&mut sqlite).unwrap();
        let _report = conn.transact(&mut sqlite, r#"[
            {  :db/ident       :foo/bar
               :db/valueType   :db.type/long
               :db/cardinality :db.cardinality/one },
            {  :db/ident       :foo/baz
               :db/valueType   :db.type/boolean
               :db/cardinality :db.cardinality/one },
            {  :db/ident       :foo/bap
               :db/valueType   :db.type/string
               :db/cardinality :db.cardinality/many}]"#).expect("transaction expected to succeed");
        let _report = conn.transact(&mut sqlite, r#"[
            {  :foo/bar        100
               :foo/baz        false
               :foo/bap        ["one","two","buckle my shoe"] },
            {  :foo/bar        200
               :foo/baz        true
               :foo/bap        ["three", "four", "knock at my door"] }]"#).expect("transaction expected to succeed");
        (conn, sqlite)
    }

    fn assert_values_present_for_attribute(attribute_cache: &mut AttributeCacher, attribute: &KnownEntid, values: Vec<Vec<TypedValue>>) {
        let cached_values: Vec<Vec<TypedValue>> = attribute_cache.get(&attribute.0)
            .expect("Expected cached values")
            .values()
            .cloned()
            .collect();
        assert_eq!(cached_values, values);
    }

    #[test]
    fn test_add_to_cache() {
        let (conn, sqlite) = populate_db();
        let schema = conn.current_schema();
        let mut attribute_cache = AttributeCacher::new();
        let kw = kw!(:foo/bar);
        let entid = schema.get_entid(&kw).expect("Expected entid for attribute");
        attribute_cache.register_attribute(&sqlite, entid.0.clone() ).expect("No errors on add to cache");
        assert_values_present_for_attribute(&mut attribute_cache, &entid, vec![vec![TypedValue::Long(100)], vec![TypedValue::Long(200)]]);
    }

    #[test]
    fn test_add_attribute_already_in_cache() {
        let (conn, mut sqlite) = populate_db();
        let schema = conn.current_schema();

        let kw = kw!(:foo/bar);
        let entid = schema.get_entid(&kw).expect("Expected entid for attribute");
        let mut attribute_cache = AttributeCacher::new();

        attribute_cache.register_attribute(&mut sqlite, entid.0.clone()).expect("No errors on add to cache");
        assert_values_present_for_attribute(&mut attribute_cache, &entid, vec![vec![TypedValue::Long(100)], vec![TypedValue::Long(200)]]);
        attribute_cache.register_attribute(&mut sqlite, entid.0.clone()).expect("No errors on add to cache");
        assert_values_present_for_attribute(&mut attribute_cache, &entid, vec![vec![TypedValue::Long(100)], vec![TypedValue::Long(200)]]);
    }

    #[test]
    fn test_remove_from_cache() {
        let (conn, mut sqlite) = populate_db();
        let schema = conn.current_schema();

        let kwr = kw!(:foo/bar);
        let entidr = schema.get_entid(&kwr).expect("Expected entid for attribute");
        let kwz = kw!(:foo/baz);
        let entidz = schema.get_entid(&kwz).expect("Expected entid for attribute");

        let mut attribute_cache = AttributeCacher::new();

        attribute_cache.register_attribute(&mut sqlite, entidr.0.clone()).expect("No errors on add to cache");
        assert_values_present_for_attribute(&mut attribute_cache, &entidr, vec![vec![TypedValue::Long(100)], vec![TypedValue::Long(200)]]);
        attribute_cache.register_attribute(&mut sqlite, entidz.0.clone()).expect("No errors on add to cache");
        assert_values_present_for_attribute(&mut attribute_cache, &entidz, vec![vec![TypedValue::Boolean(false)], vec![TypedValue::Boolean(true)]]);

        // test that we can remove an item from cache
        attribute_cache.deregister_attribute(&entidz.0).expect("No errors on remove from cache");
        assert_eq!(attribute_cache.get(&entidz.0), None);
    }

    #[test]
    fn test_remove_attribute_not_in_cache() {
        let (conn, _sqlite) = populate_db();
        let mut attribute_cache = AttributeCacher::new();

        let schema = conn.current_schema();
        let kw = kw!(:foo/baz);
        let entid = schema.get_entid(&kw).expect("Expected entid for attribute").0;
        assert_eq!(None, attribute_cache.deregister_attribute(&entid));
    }

    #[test]
    fn test_fetch_attribute_value_for_entid() {
        let (conn, mut sqlite) = populate_db();
        let schema = conn.current_schema();

        let entities = conn.q_once(&sqlite, r#"[:find ?e . :where [?e :foo/bar 100]]"#, None).expect("Expected query to work").into_scalar().expect("expected scalar results");
        let entid = match entities {
            Some(TypedValue::Ref(entid)) => entid,
            x => panic!("expected Some(Ref), got {:?}", x),
        };

        let kwr = kw!(:foo/bar);
        let attr_entid = schema.get_entid(&kwr).expect("Expected entid for attribute").0;

        let mut attribute_cache = AttributeCacher::new();

        attribute_cache.register_attribute(&mut sqlite, attr_entid.clone()).expect("No errors on add to cache");
        let val = attribute_cache.get_value_for_entid(&attr_entid, &entid).expect("Expected value");
        assert_eq!(*val, TypedValue::Long(100));
    }

    #[test]
    fn test_fetch_attribute_values_for_entid() {
        let (conn, mut sqlite) = populate_db();
        let schema = conn.current_schema();

        let entities = conn.q_once(&sqlite, r#"[:find ?e . :where [?e :foo/bar 100]]"#, None).expect("Expected query to work").into_scalar().expect("expected scalar results");
        let entid = match entities {
            Some(TypedValue::Ref(entid)) => entid,
            x => panic!("expected Some(Ref), got {:?}", x),
        };

        let kwp = kw!(:foo/bap);
        let attr_entid = schema.get_entid(&kwp).expect("Expected entid for attribute").0;

        let mut attribute_cache = AttributeCacher::new();

        attribute_cache.register_attribute(&mut sqlite, attr_entid.clone()).expect("No errors on add to cache");
        let val = attribute_cache.get_values_for_entid(&attr_entid, &entid).expect("Expected value");
        assert_eq!(*val, vec![TypedValue::String(Rc::new("buckle my shoe".to_string())), TypedValue::String(Rc::new("one".to_string())), TypedValue::String(Rc::new("two".to_string()))]);
    }
}


