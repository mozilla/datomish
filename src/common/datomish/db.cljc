;; Copyright 2016 Mozilla
;;
;; Licensed under the Apache License, Version 2.0 (the "License"); you may not use
;; this file except in compliance with the License. You may obtain a copy of the
;; License at http://www.apache.org/licenses/LICENSE-2.0
;; Unless required by applicable law or agreed to in writing, software distributed
;; under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
;; CONDITIONS OF ANY KIND, either express or implied. See the License for the
;; specific language governing permissions and limitations under the License.

(ns datomish.db
  #?(:cljs
     (:require-macros
      [datomish.pair-chan :refer [go-pair <?]]
      [cljs.core.async.macros :refer [go]]))
  (:require
   [clojure.set]
   [datomish.query.context :as context]
   [datomish.query.projection :as projection]
   [datomish.query.source :as source]
   [datomish.query :as query]
   [datomish.datom :as dd :refer [datom datom? #?@(:cljs [Datom])]]
   [datomish.util :as util
    #?(:cljs :refer-macros :clj :refer) [raise raise-str cond-let]]
   [datomish.schema :as ds]
   [datomish.sqlite :as s]
   [datomish.sqlite-schema :as sqlite-schema]
   [datomish.tufte-stub :as tufte
    #?(:cljs :refer-macros :clj :refer) [p]]
   #?@(:clj [[datomish.pair-chan :refer [go-pair <?]]
             [clojure.core.async :as a :refer [chan go <! >!]]])
   #?@(:cljs [[datomish.pair-chan]
              [cljs.core.async :as a :refer [chan <! >!]]]))
  #?(:clj
     (:import
      [datomish.datom Datom])))

#?(:clj
   ;; From https://stuartsierra.com/2015/05/27/clojure-uncaught-exceptions
   ;; Assuming require [clojure.tools.logging :as log]
   (Thread/setDefaultUncaughtExceptionHandler
     (reify Thread$UncaughtExceptionHandler
       (uncaughtException [_ thread ex]
         (println ex "Uncaught exception on" (.getName thread))))))

(def max-sql-vars 999)    ;; TODO: generalize.


;; ----------------------------------------------------------------------------
;; define data-readers to be made available to EDN readers. in CLJS
;; they're magically available. in CLJ, data_readers.clj may or may
;; not work, but you can always simply do
;;
;;  (clojure.edn/read-string {:readers datomish/data-readers} "...")
;;

(defonce -id-literal-idx (atom -1000000))

(defrecord TempId [part idx])

(defn id-literal
  ([part]
   (if (sequential? part)
     (apply id-literal part)
     (->TempId part (swap! -id-literal-idx dec))))
  ([part idx]
   (->TempId part idx)))

(defn id-literal? [x]
  (instance? TempId x))

(defrecord LookupRef [a v])

(defn lookup-ref
  [a v]
  (if (and
        (or (keyword? a)
            (integer? a))
        v)
    (->LookupRef a v)
    (raise (str "Lookup-ref with bad attribute " a " or value " v
                {:error :transact/bad-lookup-ref, :a a, :v v}))))

(defn lookup-ref? [x]
  "Return `x` if `x` is like [:attr value], nil otherwise."
  (when (instance? LookupRef x)
    x))

(defprotocol IClock
  (now
    [clock]
    "Return integer milliseconds since the Unix epoch."))

(defprotocol IDB
  (query-context
    [db])

  (close-db
    [db]
    "Close this database. Returns a pair channel of [nil error].")

  (schema
    [db]
    "Return the schema of this database.")

  (entid
    [db ident]
    "Returns the entity id associated with a symbolic keyword, or the id itself if passed.")

  (ident
    [db eid]
    "Returns the keyword associated with an id, or the key itself if passed.")

  (part-map
    [db]
    "Return the partition map of this database, like {:db.part/user {:start 0x100 :idx 0x101}, ...}.")

  (in-transaction!
    [db chan-fn]
    "Evaluate the given `chan-fn` in an exclusive transaction. If it returns non-nil,
    commit the transaction; otherwise, rollback the transaction.

    `chan-fn` should be a function of no arguments returning a pair-chan.

    Returns a pair-chan resolving to the same pair as the pair-chan returned by `chan-fn`.")

  (<bootstrapped? [db]
    "Return true if this database has no transactions yet committed.")

  (<avs
    [db avs]
    "Search for many matching datoms using the AVET index.

    Take [[a0 v0] [a1 v1] ...] and return a map {[a0 v0] e0}.  If no datom [e1 a1 v1] exists, the
    key [a1 v1] is not present in the returned map.")

  (<apply-entities
    [db tx entities]
    "Apply entities to the store, returning sequence of datoms transacted.")

  (<apply-db-ident-assertions
    [db added-idents retracted-idents]
    "Apply added and retracted idents to the store, using `merge` as a `merge-with` function for additions.")

  (<apply-db-install-assertions
    [db fragment merge]
    "Apply added schema fragment to the store, using `merge` as a `merge-with` function.")

  (<apply-db-alter-assertions
    [db altered-attributes])

  (<apply-db-part-map
    [db part-map]
    "Apply updated partition map."))

(defn db? [x]
  (and (satisfies? IDB x)
       (satisfies? IClock x)))

(defn- row->Datom [schema row]
  (let [e (:e row)
        a (:a row)
        v (:v row)]
    (Datom. e a (ds/<-SQLite schema a v) (:tx row) (and (some? (:added row)) (not= 0 (:added row))))))

(defn- <insert-fulltext-value [db value]
  (go-pair
    ;; This dance is necessary to keep fulltext_values.text unique.  We want uniqueness so that we
    ;; can work with string values, maintaining consistency throughout the transactor
    ;; implementation.  (Without this, we'd need to handle a [rowid text] pair specially when
    ;; comparing in the transactor.)  Unfortunately, it's not possible to declare a unique
    ;; constraint on a virtual table, including an FTS table.  External content tables (see
    ;; http://www.sqlite.org/fts3.html#section_6_2_2) don't appear to address our use case, so we
    ;; maintain uniqueness ourselves.
    (let [rowid
          (if-let [row (first (<? (s/all-rows (:sqlite-connection db) ["SELECT rowid FROM fulltext_values WHERE text = ?" value])))]
            (:rowid row)
            (do
              (<? (s/execute! (:sqlite-connection db) ["INSERT INTO fulltext_values VALUES (?)" value]))
              (:rowid (first (<? (s/all-rows (:sqlite-connection db) ["SELECT last_insert_rowid() AS rowid"]))))))
          ]
      rowid)))

(defn datoms-attribute-transform
  [db x]
  {:pre [(db? db)]}
  (entid db x))

(defn datoms-constant-transform
  [db x]
  {:pre [(db? db)]}
  (sqlite-schema/->SQLite x))

(defn datoms-source [db]
  (source/map->DatomsSource
    {:table :datoms
     :schema (:schema db)
     :fulltext-table :fulltext_datoms
     :fulltext-values :fulltext_values
     :fulltext-view :all_datoms
     :columns [:e :a :v :tx :added]
     :attribute-transform (partial datoms-attribute-transform db)
     :constant-transform (partial datoms-constant-transform db)
     :table-alias source/gensym-table-alias
     :make-constraints nil}))

(defn- retractAttributes->queries [oeas tx]
  (let [where-part
        "(e = ? AND a = ?)"

        repeater (memoize (fn [n] (interpose " OR " (repeat n where-part))))]
    (map
      (fn [chunk]
        (cons
          (apply str
                 "INSERT INTO temp.tx_lookup_after (e0, a0, v0, tx0, added0, value_type_tag0, sv, svalue_type_tag,
                  rid, e, a, v, tx, value_type_tag)
                  SELECT e, a, v, ?, 0, value_type_tag, v, value_type_tag,
                  rowid, e, a, v, ?, value_type_tag
                  FROM datoms
                  WHERE "
                 (repeater (count chunk)))
          (cons
            tx
            (cons
              tx
              (mapcat (fn [[_ e a]]
                        [e a])
                      chunk)))))
      (partition-all (quot (- max-sql-vars 2) 2) oeas))))

(defn- retractEntities->queries [oes tx]
  (let [ref-tag (sqlite-schema/->tag :db.type/ref)

        ;; TODO: include index_vaet flag here, so we can use that index to speed up the deletion.
        where-part
        (str "e = ? OR (v = ? AND value_type_tag = " ref-tag ")") ;; Retract the entity and all refs to the entity.

        repeater (memoize (fn [n] (interpose " OR " (repeat n where-part))))]
    (map
      (fn [chunk]
        (cons
          (apply str
                 "INSERT INTO temp.tx_lookup_after (e0, a0, v0, tx0, added0, value_type_tag0, sv, svalue_type_tag,
                  rid, e, a, v, tx, value_type_tag)
                  SELECT e, a, v, ?, 0, value_type_tag, v, value_type_tag,
                  rowid, e, a, v, ?, value_type_tag
                  FROM datoms
                  WHERE "
                 (repeater (count chunk)))
          (cons
            tx
            (cons
              tx
              (mapcat (fn [[_ e]]
                        [e e])
                      chunk)))))
      (partition-all (quot (- max-sql-vars 2) 2) oes))))

(defn- retractions->queries [retractions tx fulltext? ->SQLite]
  (let
      [f-q
       "WITH vv AS (SELECT rowid FROM fulltext_values WHERE text = ?)
     INSERT INTO temp.tx_lookup_before (e0, a0, v0, tx0, added0, value_type_tag0, sv, svalue_type_tag)
     VALUES (?, ?, (SELECT rowid FROM vv), ?, 0, ?, (SELECT rowid FROM vv), ?)"

       non-f-q
       "INSERT INTO temp.tx_lookup_before (e0, a0, v0, tx0, added0, value_type_tag0, sv, svalue_type_tag)
     VALUES (?, ?, ?, ?, 0, ?, ?, ?)"]
    (map
      (fn [[_ e a v]]
        (let [[v tag] (->SQLite a v)]
          (if (fulltext? a)
            [f-q
             v e a tx tag tag]
            [non-f-q
             e a v tx tag v tag])))
      retractions)))

(defn- non-fts-many->queries [ops tx ->SQLite indexing? ref? unique?]
  (let [q "INSERT INTO temp.tx_lookup_before (e0, a0, v0, tx0, added0, value_type_tag0, index_avet0, index_vaet0, index_fulltext0, unique_value0, sv, svalue_type_tag) VALUES "

        values-part
        ;; e0, a0, v0, tx0, added0, value_type_tag0
        ;; index_avet0, index_vaet0, index_fulltext0,
        ;; unique_value0, sv, svalue_type_tag
        "(?, ?, ?, ?, 1, ?, ?, ?, 0, ?, ?, ?)"

        repeater (memoize (fn [n] (interpose ", " (repeat n values-part))))]

    ;; This query takes ten variables per item. So we partition into max-sql-vars / 10.
    (map
      (fn [chunk]
        (cons
          ;; Query string.
          (apply str q (repeater (count chunk)))

          ;; Bindings.
          (mapcat (fn [[_ e a v]]
                    (let [[v tag] (->SQLite a v)]
                      [e a v tx tag
                       (indexing? a)      ; index_avet
                       (ref? a)           ; index_vaet
                       (unique? a)        ; unique_value
                       v tag]))
                  chunk)))

      (partition-all (quot max-sql-vars 10) ops))))

(defn- non-fts-one->queries [ops tx ->SQLite indexing? ref? unique?]
  (let [first-values-part
        ;; TODO: order value and tag closer together.
        ;; flags0
        ;; sv, svalue_type_tag
        "(?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?)"
        first-repeater (memoize (fn [n] (interpose ", " (repeat n first-values-part))))

        second-values-part
        "(?, ?, ?, ?, ?, ?)"
        second-repeater (memoize (fn [n] (interpose ", " (repeat n second-values-part))))
        ]

    ;; :db.cardinality/one takes two queries.
    (mapcat
      (fn [chunk]
        [(cons
           (apply
             str
             "INSERT INTO temp.tx_lookup_before (e0, a0, v0, tx0, added0, value_type_tag0, index_avet0, index_vaet0, index_fulltext0, unique_value0, sv, svalue_type_tag) VALUES "
             (first-repeater (count chunk)))
           (mapcat (fn [[_ e a v]]
                     (let [[v tag] (->SQLite a v)]
                       [e a v tx 1 tag
                        (indexing? a)   ; index_avet
                        (ref? a)        ; index_vaet
                        (unique? a)     ; unique_value
                        v tag]))
                   chunk))

         (cons
           (apply
             str
             "INSERT INTO temp.tx_lookup_before (e0, a0, v0, tx0, added0, value_type_tag0) VALUES "
             (second-repeater (count chunk)))
           (mapcat (fn [[_ e a v]]
                     (let [[v tag] (->SQLite a v)]
                       [e a v tx 0 tag]))
                   chunk))])
      (partition-all (quot max-sql-vars 11) ops))))

(def initial-many-searchid 2000)    ; Just to make it more obvious in the DB.
(def initial-one-searchid  5000)

;;; An FTS insertion happens in two parts.
;;;
;;; Firstly, we ensure that the fulltext value is present in the store.
;;; This is effectively an INSERT OR IGNORE… but FTS tables don't support
;;; uniqueness constraints. So we do it through a trigger on a view.
;;;
;;; When we insert the value, we pass with it a searchid. We'll use this
;;; later when inserting the datom, then we'll throw it away. The FTS table
;;; only contains searchids for the duration of the transaction that uses
;;; them.
;;;
;;; Secondly, we insert a row just like for non-FTS. The only difference
;;; is that the value is the rowid into the fulltext_values table.
(defn- fts-many->queries [ops tx ->SQLite indexing? ref? unique?]
  ;; TODO: operations with the same text value should be
  ;; coordinated here!
  ;; It'll work fine without so long as queries are executed
  ;; in order and not combined, but even so it's inefficient.
  (conj
    (mapcat
      (fn [[_ e a v] searchid]
        (let [[v tag] (->SQLite a v)]
          ;; First query: ensure the value exists.
          [["INSERT INTO fulltext_values_view (text, searchid) VALUES (?, ?)"
            v searchid]

           ;; Second query: lookup.
           [(str
              "WITH vv(rowid) AS (SELECT rowid FROM fulltext_values WHERE searchid = ?) "
              "INSERT INTO temp.tx_lookup_before (e0, a0, v0, tx0, added0, value_type_tag0, index_avet0, index_vaet0, index_fulltext0, unique_value0, sv, svalue_type_tag) VALUES "
              "(?, ?, (SELECT rowid FROM vv), ?, 1, ?, ?, ?, 1, ?, (SELECT rowid FROM vv), ?)")
            searchid
            e a tx tag
            (indexing? a)      ; index_avet
            (ref? a)           ; index_vaet
            (unique? a)        ; unique_value
            tag]]))
      (sort-by (fn [[_ _ _ v]] v) ops) ;; Make testing easier by sorting by string values.  TODO: discuss expense.
      (range initial-many-searchid 999999999))
    ["UPDATE fulltext_values SET searchid = NULL WHERE searchid IS NOT NULL"]))

(defn fts-one->queries [ops tx ->SQLite indexing? ref? unique?]
  (conj
    (mapcat
      (fn [[_ e a v] searchid]
        (let [[v tag] (->SQLite a v)]
          ;; First query: ensure the value exists.
          [["INSERT INTO fulltext_values_view (text, searchid) VALUES (?, ?)"
            v searchid]

           ;; Second and third queries: lookup.
           [(str
              "WITH vv(rowid) AS (SELECT rowid FROM fulltext_values WHERE searchid = ?) "
              "INSERT INTO temp.tx_lookup_before (e0, a0, v0, tx0, added0, value_type_tag0, index_avet0, index_vaet0, index_fulltext0, unique_value0, sv, svalue_type_tag) VALUES "
              "(?, ?, (SELECT rowid FROM vv), ?, 1, ?, ?, ?, 1, ?, (SELECT rowid FROM vv), ?)")
            searchid
            e a tx tag
            (indexing? a)      ; index_avet
            (ref? a)           ; index_vaet
            (unique? a)        ; unique_value
            tag]

           [(str
              "INSERT INTO temp.tx_lookup_before (e0, a0, v0, tx0, added0, value_type_tag0) VALUES "
              "(?, ?, (SELECT rowid FROM fulltext_values WHERE searchid = ?), ?, 0, ?)")
            e a searchid tx tag]]))
      (sort-by (fn [[_ _ _ v]] v) ops) ;; Make testing easier by sorting by string values.  TODO: discuss expense.
      (range initial-one-searchid 999999999))
    ["UPDATE fulltext_values SET searchid = NULL WHERE searchid IS NOT NULL"]))

(defn- -run-queries [conn queries exception-message]
  (go-pair
    (try
      (doseq [q queries]
        (<? (s/execute! conn q)))
      (catch #?(:clj java.lang.Exception :cljs js/Error) e
        (throw (ex-info exception-message {} e))))))

(defn- -preamble-drop [conn]
  (go-pair
    (p :preamble
       (doseq [q [;; XXX ["DROP INDEX IF EXISTS temp.idx_tx_lookup_before_added"]
                  (sqlite-schema/create-temp-tx-lookup-statement "temp.tx_lookup_before")
                  (sqlite-schema/create-temp-tx-lookup-statement "temp.tx_lookup_after")
                  ;; TODO: move later, into -build-transaction.
                  ;; temp goes on index name, not table name.  See http://stackoverflow.com/a/22308016.
                  (sqlite-schema/create-temp-tx-lookup-eavt-statement "temp.idx_tx_lookup_before_eavt" "tx_lookup_before")
                  (sqlite-schema/create-temp-tx-lookup-eavt-statement "temp.idx_tx_lookup_after_eavt" "tx_lookup_after")
                  ["DELETE FROM temp.tx_lookup_before"]
                  ["DELETE FROM temp.tx_lookup_after"]]]
         (<? (s/execute! conn q))))))

(defn- -after-drop [conn]
  (go-pair
    (p :postamble
       ;; TODO: delete tx_lookup_before after filling tx_lookup_after.
       (doseq [q [;; XXX ["DROP INDEX IF EXISTS temp.idx_tx_lookup_before_added"]
                  ["DROP INDEX IF EXISTS temp.idx_tx_lookup_before_eavt"]
                  ["DROP INDEX IF EXISTS temp.idx_tx_lookup_after_eavt"]
                  ["DELETE FROM temp.tx_lookup_before"]
                  ["DELETE FROM temp.tx_lookup_after"]]]
         (<? (s/execute! conn q))))))

(defn- -build-transaction [conn tx]
  (let [build-indices ["CREATE INDEX IF NOT EXISTS temp.idx_tx_lookup_added ON tx_lookup_before (added0)"]

        ;; First is fast, only one table walk: lookup by exact eav.
        ;; Second is slower, but still only one table walk: lookup old value by ea.
        insert-into-tx-lookup
        ["INSERT INTO temp.tx_lookup_after
          SELECT t.e0, t.a0, t.v0, t.tx0, t.added0, t.value_type_tag0, t.index_avet0, t.index_vaet0, t.index_fulltext0, t.unique_value0, t.sv, t.svalue_type_tag, d.rowid, d.e, d.a, d.v, d.tx, d.value_type_tag
          FROM temp.tx_lookup_before AS t
          LEFT JOIN datoms AS d
          ON t.e0 = d.e AND
             t.a0 = d.a AND
             t.sv = d.v AND
             t.svalue_type_tag = d.value_type_tag AND
             t.sv IS NOT NULL

          UNION ALL
          SELECT t.e0, t.a0, t.v0, t.tx0, t.added0, t.value_type_tag0, t.index_avet0, t.index_vaet0, t.index_fulltext0, t.unique_value0, t.sv, t.svalue_type_tag, d.rowid, d.e, d.a, d.v, d.tx, d.value_type_tag
          FROM temp.tx_lookup_before AS t,
               datoms AS d
          WHERE t.sv IS NULL AND
                t.e0 = d.e AND
                t.a0 = d.a"]

        t-datoms-not-already-present
        ["INSERT INTO transactions (e, a, v, tx, added, value_type_tag)
          SELECT e0, a0, v0, ?, 1, value_type_tag0
          FROM temp.tx_lookup_after
          WHERE added0 IS 1 AND e IS NULL" tx]

        t-retract-datoms-carefully
        ["INSERT INTO transactions (e, a, v, tx, added, value_type_tag)
          SELECT e, a, v, ?, 0, value_type_tag
          FROM temp.tx_lookup_after
          WHERE added0 IS 0 AND ((sv IS NOT NULL) OR (sv IS NULL AND v0 IS NOT v)) AND v IS NOT NULL" tx]
        ]
    (go-pair
      (doseq [q [build-indices insert-into-tx-lookup
                 t-datoms-not-already-present
                 t-retract-datoms-carefully]]
        (<? (s/execute! conn q))))))

(defn- -build-datoms [conn tx]
  (let [d-datoms-not-already-present
        ["INSERT INTO datoms (e, a, v, tx, value_type_tag, index_avet, index_vaet, index_fulltext, unique_value)
          SELECT e0, a0, v0, ?, value_type_tag0,
          index_avet0, index_vaet0, index_fulltext0, unique_value0
          FROM temp.tx_lookup_after
          WHERE added0 IS 1 AND e IS NULL" tx]

        ;; TODO: retract fulltext datoms correctly.
        d-retract-datoms-carefully
        ["WITH ids AS (SELECT l.rid FROM temp.tx_lookup_after AS l WHERE l.added0 IS 0 AND ((l.sv IS NOT NULL) OR (l.sv IS NULL AND l.v0 IS NOT l.v)))
         DELETE FROM datoms WHERE rowid IN ids"
         ]]
    (-run-queries conn [d-datoms-not-already-present d-retract-datoms-carefully]
                  "Transaction violates unique constraint")))

(defn- -<apply-entities [db tx entities]
  (let [schema         (.-schema db)
        ->SQLite       (partial ds/->SQLite schema)
        fulltext?      (memoize (partial ds/fulltext? schema))
        many?          (memoize (fn [a] (ds/multival? schema a)))
        indexing?      (memoize (fn [a] (ds/indexing? schema a)))
        ref?           (memoize (fn [a] (ds/ref? schema a)))
        unique?        (memoize (fn [a] (ds/unique? schema a)))
        conn           (:sqlite-connection db)

        ;; Collect all the queries we need to run.
        queries (atom [])
        operations (group-by first entities)]

    ;; Belt and braces.  At this point, we should have already errored out if op is not known.
    (let [known #{:db/retract :db/add :db.fn/retractAttribute :db.fn/retractEntity}]
      (when-not (clojure.set/subset? (keys operations) known)
        (let [unknown (apply dissoc operations known)]
          (raise (str "Unknown operations " (apply sorted-set (keys unknown)))
                 {:error :transact/syntax, :operations (apply sorted-set (keys unknown))}))))

    ;; We can turn all non-FTS operations into simple SQL queries that we run serially.
    ;; FTS queries require us to get a rowid from the FTS table and use that for
    ;; insertion, so we need another pass.
    ;; We can't just freely use `go-pair` here, because this function is so complicated
    ;; that ClojureScript blows the stack trying to compile it.

    (when-let [eas (:db.fn/retractAttribute operations)]
      (swap!
        queries concat (retractAttributes->queries eas tx)))

    (when-let [es (:db.fn/retractEntity operations)]
      (swap!
        queries concat (retractEntities->queries es tx)))

    (when-let [retractions (:db/retract operations)]
      (swap!
        queries concat (retractions->queries retractions tx fulltext? ->SQLite)))

    ;; We want to partition our additions into four groups according to two
    ;; characteristics: whether they require writing to the FTS value table,
    ;; and whether the attribute has a 'many' cardinality constraint. Each of
    ;; these four requires different queries.
    (let [additions
          (group-by (fn [[op e a v]]
                      (if (fulltext? a)
                        (if (many? a)
                          :fts-many
                          :fts-one)
                        (if (many? a)
                          :non-fts-many
                          :non-fts-one)))
                    (:db/add operations))
          transforms
          {:fts-one fts-one->queries
           :fts-many fts-many->queries
           :non-fts-one non-fts-one->queries
           :non-fts-many non-fts-many->queries}]

      (doseq [[key ops] additions]
        (when-let [transform (key transforms)]
          (swap!
            queries concat
            (transform ops tx ->SQLite indexing? ref? unique?)))))

    ;; Now run each query.
    ;; This code is a little tortured to avoid blowing the compiler stack in cljs.

    (go-pair
      (<? (-preamble-drop conn))

      (p :run-insert-queries
         (<? (-run-queries conn @queries "Transaction violates cardinality constraint")))

      ;; Follow up by building indices, then do the work.
      (p :build-and-transaction
         (<? (-build-transaction conn tx)))

      (p :update-materialized-datoms
         (<? (-build-datoms conn tx)))

      (<? (-after-drop conn))

      ;; Return the written transaction.
      (p :select-tx-data
         (mapv (partial row->Datom schema)
               (<?
                 (s/all-rows
                   (:sqlite-connection db)
                   ;; We index on tx, so the following is fast.
                   ["SELECT * FROM transactions WHERE tx = ?" tx])))))))

(defn symbolic-schema->schema [db symbolic-schema]
  (ds/schema (into {} (map (fn [[k v]] [(entid db k) v]) symbolic-schema))))

(defrecord DB [sqlite-connection schema ident-map part-map]
  ;; ident-map maps between keyword idents and integer entids.  The set of idents and entids is
  ;; disjoint, so we represent both directions of the mapping in the same map for simplicity.  Also
  ;; for simplicity, we assume that an entid has at most one associated ident, and vice-versa.  See
  ;; http://docs.datomic.com/identity.html#idents.
  ;;
  ;; The partition-map part-map looks like {:db.part/user {:start 0x100 :idx 0x101}, ...}.  It maps
  ;; between keyword ident part names and integer ranges, where start is the beginning of the
  ;; range (for future use to help identify which partition entids lie in, and idx is the current
  ;; maximum entid in the partition.

  IDB
  (query-context [db] (context/make-context (datoms-source db)))

  (schema [db] (.-schema db))

  (entid [db ident]
    (if (keyword? ident)
      (get (.-ident-map db) ident ident)
      ident))

  (ident [db eid]
    (if-not (keyword? eid)
      (get (.-ident-map db) eid eid)
      eid))

  (part-map [db]
    (:part-map db))

  (in-transaction! [db chan-fn]
    (s/in-transaction!
      (:sqlite-connection db) chan-fn))

  (<bootstrapped? [db]
    (go-pair
      (->
        (:sqlite-connection db)
        (s/all-rows ["SELECT EXISTS(SELECT 1 FROM transactions LIMIT 1) AS bootstrapped"])
        (<?)
        (first)
        (:bootstrapped)
        (not= 0))))

  (<avs
    [db avs]
    {:pre [(sequential? avs)]}

    (go-pair
      (let [schema
            (.-schema db)

            values-part
            "(?, ?, ?, ?)"

            repeater
            (memoize (fn [n] (interpose ", " (repeat n values-part))))

            exec
            (partial s/execute! (:sqlite-connection db))

            ;; Map [a v] -> searchid.
            av->searchid
            (into {} (map vector avs (range)))

            ;; Each query takes 4 variables per item. So we partition into max-sql-vars / 4.
            qs
            (map
              (fn [chunk]
                (cons
                  ;; Query string.
                  (apply str "WITH t(searchid, a, v, value_type_tag) AS (VALUES "
                         (apply str (repeater (count chunk))) ;; TODO: join?
                         ") SELECT t.searchid, d.e
                           FROM t, all_datoms AS d
                           WHERE d.index_avet IS NOT 0 AND d.a = t.a AND d.value_type_tag = t.value_type_tag AND d.v = t.v")

                  ;; Bindings.
                  (mapcat (fn [[[a v] searchid]]
                            (let [a (entid db a)
                                  [v tag] (ds/->SQLite schema a v)]
                              [searchid a v tag]))
                          chunk)))

              (partition-all (quot max-sql-vars 4) av->searchid))

            ;; Map searchid -> e.  There's a generic reduce that takes [pair-chan] lurking in here.
            searchid->e
            (loop [coll (transient {})
                   qs qs]
              (let [[q & qs] qs]
                (if q
                  (let [rs (<? (s/all-rows (:sqlite-connection db) q))
                        coll* (reduce conj! coll (map (juxt :searchid :e) rs))]
                    (recur coll* qs))
                  (persistent! coll))))
            ]
        (util/mapvals (partial get searchid->e) av->searchid))))

  (<apply-entities [db tx entities]
    {:pre [(db? db) (sequential? entities)]}
    (-<apply-entities db tx entities))

  (<apply-db-part-map [db part-map]
    (go-pair
      (let [exec (partial s/execute! (:sqlite-connection db))]
        (let [pairs (mapcat (fn [[part {:keys [start idx]}]]
                              (when-not (= idx (get-in db [:part-map part :idx]))
                                [(sqlite-schema/->SQLite part) idx]))
                            part-map)]
          ;; TODO: chunk into 999/2 sections, for safety.
          (when-not (empty? pairs)
            (<?
              (exec
                (cons (apply str "UPDATE parts SET idx = CASE"
                             (concat
                               (repeat (count pairs) " WHEN part = ? THEN ?")
                               [" ELSE idx END"]))
                      pairs))))))
      (assoc db :part-map part-map)))

  (<apply-db-ident-assertions [db added-idents retracted-idents]
    (go-pair
      (if (and (empty? added-idents)
               (empty? retracted-idents))
        db

        ;; We have a bunch of additions and a bunch of retractions.
        ;; Some of these will pair up, indicating a rename.
        ;;
        ;; We flip the incoming maps to get eid->ident, then we find
        ;; the renames, pure additions, and pure retractions.
        ;;
        ;; We delete the retracted idents, insert the added idents,
        ;; and update the renames.
        ;;
        ;; Finally, we update the :ident-map and :symbolic-schema
        ;; accordingly.
        (let [inverted-additions (clojure.set/map-invert added-idents)
              inverted-retractions (clojure.set/map-invert retracted-idents)
              renamed-eids (clojure.set/intersection (set (keys inverted-retractions))
                                                     (set (keys inverted-additions)))
              pure-additions (apply dissoc inverted-additions renamed-eids)
              pure-retractions (apply dissoc inverted-retractions renamed-eids)]

          (let [exec (partial s/execute! (:sqlite-connection db))]
            ;; We're about to delete then recreate an ident.
            ;; That might violate foreign key constraints, so we defer constraint
            ;; checking for the duration of this transaction.
            (when-not (empty? renamed-eids)
              (<? (exec ["PRAGMA defer_foreign_keys = 1"])))

            ;; TODO: batch insert and delete.
            (doseq [[entid ident] pure-retractions]
              (when-not (contains? renamed-eids entid)
                (<? (exec
                      ["DELETE FROM idents WHERE ident = ? AND entid = ?" (sqlite-schema/->SQLite ident) entid]))))

            (doseq [[entid ident] pure-additions]
              (when-not (contains? renamed-eids entid)
                (<? (exec
                      ["INSERT INTO idents VALUES (?, ?)" (sqlite-schema/->SQLite ident) entid]))))

            ;; Renames.
            (let [renames
                  (into (sorted-map)
                        (map (fn [eid] [(get inverted-retractions eid)
                                        (get inverted-additions eid)])
                             renamed-eids))]
              (doseq [[from to] renames]
                (let [from (sqlite-schema/->SQLite from)
                      to (sqlite-schema/->SQLite to)]
                  (<? (exec ["UPDATE schema SET ident = ? WHERE ident = ?"
                             to from]))
                  (<? (exec ["UPDATE idents SET ident = ? WHERE ident = ?"
                             to from]))))

              (-> db
                ;; Remove retractions -- eid and ident -- from the ident map.
                (util/dissoc-from :ident-map (concat (vals pure-retractions)
                                                     (keys pure-retractions)))
                ;; Remove idents from the schema.
                (util/dissoc-from :symbolic-schema (vals pure-retractions))

                ;; Rename renamed attributes in the schema.
                (update :symbolic-schema clojure.set/rename-keys renames)

                ;; Remove old idents (and, coincidentally, 'from' idents for renames).
                (update :ident-map (fn [m] (apply dissoc m (keys renames))))

                ;; Add new ones, and the results of renames.
                (update :ident-map (fn [m] (merge m added-idents)))
                (update :ident-map (fn [m] (merge m (clojure.set/map-invert added-idents)))))))))))

  (<apply-db-install-assertions [db fragment merge]
    (go-pair
      (let [schema         (.-schema db)
            ->SQLite       (partial ds/->SQLite schema)
            exec (partial s/execute! (:sqlite-connection db))]
        ;; TODO: batch insert.
        (doseq [[ident attr-map] fragment]
          (doseq [[attr value] attr-map]
            ;; This is a little sloppy.  We need to store idents as entids, since they're (mostly)
            ;; :db.type/ref attributes.  So we use that entid passes through idents it doesn't
            ;; recognize, and assuming that we have no :db.type/keyword values that match idents.
            ;; This is safe for now.
            (let [[v tag] (->SQLite (entid db attr) (entid db value))]
              (<? (exec
                    ["INSERT INTO schema VALUES (?, ?, ?, ?)"
                     (sqlite-schema/->SQLite ident) (sqlite-schema/->SQLite attr)
                     v tag]))))))

      (let [symbolic-schema (merge-with merge (:symbolic-schema db) fragment)
            schema          (symbolic-schema->schema db symbolic-schema)]
        (assoc db
               :symbolic-schema symbolic-schema
               :schema schema))))

  (<apply-db-alter-assertions [db altered-attributes]
    ;; altered-attributes is a sequence of [e a v].
    ;; Note that the 'e' is the attribute being altered, and the 'a's
    ;; are the attribute's attributes!
    (if (empty? altered-attributes)
      (go-pair db)
      (let [schema (.-schema db)

            exec (partial s/execute! (:sqlite-connection db))
            run (partial s/all-rows (:sqlite-connection db))

            update-schema
            (fn [v ent attr]
              ["UPDATE schema SET value = ? WHERE ident = ? AND attr = ?"
               v
               (sqlite-schema/->SQLite ent)
               (sqlite-schema/->SQLite attr)])

            alter-eav
            (fn [{:keys [checks statements symbolic-schema :as acc]} [e a v]]
              (let [ent (ident db e)
                    attr (ident db a)

                    new (if (= (ds/valueType schema a) :db.type/ref)
                          (ident db v)
                          (ds/<-SQLite schema a v))
                    old (get-in symbolic-schema [ent attr])

                    datoms-table (if (ds/fulltext? schema e)
                                   "fulltext_datoms"
                                   "datoms")]

                ;; Future:
                ;; :db/index => set index_avet.
                ;; Change valueType to ref => set index_vaet.
                ;; Add fulltext => set index_fulltext.
                (if (= old new)
                  acc
                  (case attr

                    (:db/noHistory :db/isComponent)
                    ;; These values are booleans and don't affect the DB.
                    {:checks checks
                     :statements
                     (conj statements (update-schema v ent attr))
                     :symbolic-schema
                     (assoc-in symbolic-schema [ent attr] (== 1 new))}

                    :db/cardinality
                    (cond
                      (and (= old :db.cardinality/one)
                           (= new :db.cardinality/many))

                      ;; See the comment in <apply-db-install-assertions for how we're
                      ;; passing through idents as strings here.
                      ;; Note that we're also assuming that all values for a given schema
                      ;; attribute have the same type tag.
                      {:checks checks
                       :statements
                       (conj statements (update-schema v ent attr))
                       :symbolic-schema
                       (assoc-in symbolic-schema [ent attr] new)}

                      (and (= old :db.cardinality/many)
                           (= new :db.cardinality/one))

                      ;; There is no SQLite consistency constraint that requires a single
                      ;; value for a given (e, a) pair. So when we convert a multi-valued
                      ;; attribute to a single-valued (cardinality one) attribute, we need
                      ;; to run a query.
                      ;; We collect these in 'checks', and see if they return any results
                      ;; before we change any data.
                      {:checks
                       (conj checks
                             ;; In this context we're looking for two datoms which
                             ;; share an entity, both have the right attribute ('e',
                             ;; which is a little confusing), but aren't the same row.
                             [[ent :db/cardinality]
                              [(str "SELECT EXISTS(SELECT y.v FROM "
                                    datoms-table " x, "
                                    datoms-table " y WHERE x.e = y.e AND x.a = ? AND y.a = ? AND x.rowid < y.rowid) AS yes") e e]])
                       :statements
                       (conj statements (update-schema v ent attr))
                       :symbolic-schema
                       (assoc-in symbolic-schema [ent attr] new)}

                      :else
                      (raise "Unknown cardinality" new {:error :transact/bad-cardinality :value new}))

                    :db/unique
                    (cond
                      (nil? new)
                      {:checks checks
                       :statements
                       (conj statements
                             [(str "UPDATE " datoms-table
                                   " SET unique_value = 0 WHERE a = ?") attr]
                             (update-schema v ent attr))
                       :symbolic-schema
                       (update-in symbolic-schema [ent] dissoc attr)}

                      (or
                        (and (= old :db.unique/identity)
                             (= new :db.unique/value))
                        (and (= old :db.unique/value)
                             (= new :db.unique/identity)))
                      {:checks checks
                       :statements
                       (conj statements (update-schema v ent attr))

                       :symbolic-schema
                       (assoc-in symbolic-schema [ent attr] new)}

                      ;; TODO: enabling uniqueness => set unique_value = 1,
                      ;; with checks.
                      :else
                      (raise "Unknown or unsupported uniqueness constraint" new {:error :transact/bad-unique :value new}))

                    :else
                    (raise "Unsupported attribute to alter" attr {:error :transact/bad-alter-attribute :attr attr})))))

            {:keys [checks statements symbolic-schema]}
            (reduce alter-eav
                    {:checks []
                     :statements []
                     :symbolic-schema (:symbolic-schema db)}
                    altered-attributes)]
        (go-pair
          (doseq [[[ent prop] check] checks]
            (let [r (<? (run check))]
              (when (= 1 (:yes (first r)))
                (raise "Can't alter " prop " for attribute " ent ": data not suitable."
                       {:entity ent
                        :property prop
                        :query check}))))

          (doseq [statement statements]
            (<? (exec statement)))

          ;; We need to rebuild the entid-based schema, then reverse that into
          ;; the reverse schema, so that operations like `multival?` work.
          ;; This is simpler than updating all three schema parts in place, but
          ;; more expensive.
          ;;
          ;; TODO: refactor some of this out so we can make rschema private again.
          (let [non-symbolic (symbolic-schema->schema db symbolic-schema)
                rschema (ds/rschema non-symbolic)]
            (assoc db
                   :symbolic-schema symbolic-schema
                   :schema non-symbolic
                   :rschema rschema))))))

  (close-db [db] (s/close (.-sqlite-connection db)))

  IClock
  (now [db]
    #?(:clj
       (System/currentTimeMillis)
       :cljs
       (.getTime (js/Date.)))))

(defn with-ident [db ident entid]
  (update db :ident-map #(assoc % ident entid, entid ident)))

(defn db [sqlite-connection idents parts schema]
  {:pre [(map? idents)
         (every? keyword? (keys idents))
         (map? parts)
         (every? keyword? (keys parts))
         (map? schema)
         (every? keyword? (keys schema))]}
  (let [entid-schema (ds/schema (into {} (map (fn [[k v]] [(k idents) v]) schema))) ;; TODO: fail if ident missing.
        ident-map    (into idents (clojure.set/map-invert idents))]
    (map->DB
      {:sqlite-connection sqlite-connection
       :ident-map         ident-map
       :part-map          parts
       :symbolic-schema   schema
       :schema            entid-schema
       })))

(defn reduce-error-pair [f [rv re] [v e]]
  (if re
    [nil re]
    (if e
      [nil e]
      [(f rv v) nil])))

(def default-result-buffer-size 50)

(defn <?q
  "Execute the provided query on the provided DB.
   Returns a transduced pair-chan with either:
   * One [[results] err] item (for relation and collection find specs), or
   * One [value err] item (for tuple and scalar find specs)."
  ([db find]
   (<?q db find {}))
  ([db find options]
   (let [unexpected (seq (clojure.set/difference (set (keys options)) #{:limit :order-by :inputs}))]
     (when unexpected
       (raise "Unexpected options: " unexpected {:bad-options unexpected})))
   (let [{:keys [limit order-by inputs]} options
         parsed (query/parse find)
         context (-> db
                     query-context
                     (query/options-into-context limit order-by)
                     (query/find-into-context parsed))

         ;; We turn each row into either an array of values or an unadorned
         ;; value. The row-pair-transducer does this work.
         ;; The only thing to do to handle the full suite of find specs
         ;; is to decide if we're then returning an array of transduced rows
         ;; or just the first result.
         row-pair-transducer (projection/row-pair-transducer context)
         sql (query/context->sql-string context inputs)

         first-only (context/scalar-or-tuple-query? context)
         buffer-size (if first-only
                       1
                       default-result-buffer-size)
         chan (chan buffer-size row-pair-transducer)]

     ;; Fill the channel.
     (s/<?all-rows (.-sqlite-connection db) sql chan)

     ;; If we only want the first result, great!
     ;; Otherwise, reduce it down.
     (if first-only
       chan
       (a/reduce (partial reduce-error-pair conj) [[] nil]
                 chan)))))
