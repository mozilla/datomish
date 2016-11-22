;; Copyright 2016 Mozilla
;;
;; Licensed under the Apache License, Version 2.0 (the "License"); you may not use
;; this file except in compliance with the License. You may obtain a copy of the
;; License at http://www.apache.org/licenses/LICENSE-2.0
;; Unless required by applicable law or agreed to in writing, software distributed
;; under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
;; CONDITIONS OF ANY KIND, either express or implied. See the License for the
;; specific language governing permissions and limitations under the License.

(ns datomish.js
  (:refer-clojure :exclude [])
  (:require-macros
     [datomish.pair-chan :refer [go-pair <?]]
     [datomish.promises :refer [go-promise]])
  (:require
     [datomish.util
      :as util
      :refer-macros [raise raise-str cond-let]]
     [cljs.core.async :as a :refer [take! <! >!]]
     [cljs.reader]
     [cljs-promises.core :refer [promise]]
     [datomish.cljify :refer [cljify]]
     [datomish.api :as d]
     [datomish.db :as db]
     [datomish.db-factory :as db-factory]
     [datomish.pair-chan]
     [datomish.promises :refer [take-pair-as-promise!]]
     [datomish.sqlite :as sqlite]
     [datomish.simple-schema :as simple-schema]
     [datomish.js-sqlite :as js-sqlite]
     [datomish.transact :as transact]))


;; Public API.

(def ^:export db d/db)

(defn- cljify-options [options]
  ;; Step one: basic parsing.
  (let [o (cljify options)]
    ;; Step two: convert `order-by` into keywords.
    (if-let [ord (:order-by o)]
      (assoc o
             :order-by
             (map
               (fn [[var dir]]
                 [(keyword var)
                  (case dir
                        "asc"  :asc
                        "desc" :desc
                        nil    :asc
                        :default
                        (raise "Unexpected order-by direction " dir
                               {:direction dir}))])
               ord))
      o)))

(defn ^:export q [db find options]
  (let [find (cljs.reader/read-string find)
        opts (cljify-options options)]
    (take-pair-as-promise!
      (d/<q db find opts)
      clj->js)))

(defn ^:export ensure-schema [conn simple-schema]
  (let [simple-schema (cljify simple-schema)
        datoms (simple-schema/simple-schema->schema simple-schema)]
    (take-pair-as-promise!
      (d/<transact!
        conn
        datoms)
      clj->js)))

(def js->tx-data cljify)

(def ^:export tempid (partial db/id-literal :db.part/user))

(defn ^:export transact [conn tx-data]
  ;; Expects a JS array as input.
  (try
    (let [tx-data (js->tx-data tx-data)]
      (go-promise clj->js
        (let [tx-result (<? (d/<transact! conn tx-data))
              tempids (:tempids tx-result)
              to-return (select-keys tx-result
                                     [:tempids
                                      :added-idents
                                      :added-attributes
                                      :tx
                                      :txInstant])
              jsified (clj->js to-return)]

          ;; The tempids map isn't enough for a JS caller to look up one of
          ;; these objects, so we need a lookup function.
          (aset jsified "tempid" (fn [t] (get tempids t)))
          jsified)))
    (catch js/Error e
      (println "Error in transact:" e))))

(defn ^:export open [path]
  ;; Eventually, URI.  For now, just a plain path (no file://).
  (go-promise clj->js
    (let [conn (<? (sqlite/<sqlite-connection path))
          db (<? (db-factory/<db-with-sqlite-connection conn))]
      (let [c (transact/connection-with-db db)]
        ;; We pickle the connection as a thunk here so it roundtrips through JS
        ;; without incident.
        {:conn (fn [] c)
         :db (fn [] (d/db c))
         :path path

         ;; Primary API.
         :ensureSchema (fn [simple-schema] (ensure-schema c simple-schema))
         :transact (fn [tx-data] (transact c tx-data))
         :q (fn [find opts] (q (d/db c) find opts))
         :close (fn [] (db/close-db db))

         ;; So you can generate keywords for binding in `:inputs`.
         :keyword keyword

         ;; Some helpers for testing the bridge.
         :println (fn [& xs] (apply println xs))
         :equal =
         :idx (fn [tempid] (:idx tempid))
         :cljify cljify
         :roundtrip (fn [x] (clj->js (cljify x)))

         :toString (fn [] (str "#<DB " path ">"))
         }))))
