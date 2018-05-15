/* Copyright 2018 Mozilla
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use
 * this file except in compliance with the License. You may obtain a copy of the
 * License at http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software distributed
 * under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
 * CONDITIONS OF ANY KIND, either express or implied. See the License for the
 * specific language governing permissions and limitations under the License. */

import Foundation

import MentatStore

typealias Entid = Int64

/**
 Protocol to be implemented by any object that wishes to register for transaction observation
 */
protocol Observing {
    func transactionDidOccur(key: String, reports: [TxChange])
}

/**
 Protocol to be implemented by any object that provides an interface to Mentat's transaction observers.
 */
protocol Observable {
    func register(key: String, observer: Observing, attributes: [String])
    func unregister(key: String)
}

enum CacheDirection {
    case forward;
    case reverse;
    case both;
}

/**
 The primary class for accessing Mentat's API.
 This class provides all of the basic API that can be found in Mentat's Store struct.
 The raw pointer it holds is a pointer to a Store.
*/
class Mentat: RustObject {
    fileprivate static var observers = [String: Observing]()

    /**
     Create a new Mentat with the provided pointer to a Mentat Store
     - Parameter raw: A pointer to a Mentat Store.
    */
    required override init(raw: OpaquePointer) {
        super.init(raw: raw)
    }

    /**
     Open a connection to a Store in a given location.
     If the store does not already exist, one will be created.

     - Parameter storeURI: The URI as a String of the store to open.
        If no store URI is provided, an in-memory store will be opened.
    */
    convenience init(storeURI: String = "") {
        self.init(raw: store_open(storeURI))
    }

    /**
     Add an attribute to the cache. The {@link CacheDirection} determines how that attribute can be
     looked up.

     - Parameter attribute: The attribute to cache
     - Parameter direction: The direction the attribute should be keyed.
        `FORWARD` caches values for an attribute keyed by entity
        (i.e. find values and entities that have this attribute, or find values of attribute for an entity)
        `REVERSE` caches entities for an attribute keyed by value.
        (i.e. find entities that have a particular value for an attribute).
        `BOTH` adds an attribute such that it is cached in both directions.

     - Throws: `ResultError.error` if an error occured while trying to cache the attribute.
     */
    func cache(attribute: String, direction: CacheDirection) throws {
        switch direction {
        case .forward:
            try store_cache_attribute_forward(self.raw, attribute).pointee.tryUnwrap()
        case .reverse:
            try store_cache_attribute_reverse(self.raw, attribute).pointee.tryUnwrap()
        case .both:
            try store_cache_attribute_bi_directional(self.raw, attribute).pointee.tryUnwrap()
        }
    }

    /**
    Simple transact of an EDN string.
     - Parameter transaction: The string, as EDN, to be transacted

     - Throws: `ResultError.error` if the an error occured during the transaction, or the TxReport is nil.

     - Returns: The `TxReport` of the completed transaction
    */
    func transact(transaction: String) throws -> TxReport {
        let result = store_transact(self.raw, transaction).pointee
        return TxReport(raw: try result.unwrap())
    }

    /**
     Start a new transaction.

     - Throws: `ResultError.error` if the creation of the transaction fails.
     - Throws: `ResultError.empty` if no `InProgress` is created.

     - Returns: The `InProgress` used to manage the transaction
     */
    func beginTransaction() throws -> InProgress {
        let result = store_begin_transaction(self.raw).pointee;
        return InProgress(raw: try result.unwrap())
    }

    /**
     Creates a new transaction (`InProgress`) and returns an `InProgressBuilder` for that transaction.

     - Throws: `ResultError.error` if the creation of the transaction fails.
     - Throws: `ResultError.empty` if no `InProgressBuilder` is created.

     - Returns: an `InProgressBuilder` for this `InProgress`
     */
    func entityBuilder() throws -> InProgressBuilder {
        let result = store_in_progress_builder(self.raw).pointee
        return InProgressBuilder(raw: try result.unwrap())
    }

    /**
     Creates a new transaction (`InProgress`) and returns an `EntityBuilder` for the entity with `entid`
    for that transaction.

     - Parameter entid: The `Entid` for this entity.

     - Throws: `ResultError.error` if the creation of the transaction fails.
     - Throws: `ResultError.empty` if no `EntityBuilder` is created.

     - Returns: an `EntityBuilder` for this `InProgress`
     */
    func entityBuilder(forEntid entid: Entid) throws -> EntityBuilder {
        let result = store_entity_builder_from_entid(self.raw, entid).pointee
        return EntityBuilder(raw: try result.unwrap())
    }

    /**
     Creates a new transaction (`InProgress`) and returns an `EntityBuilder` for a new entity with `tempId`
    for that transaction.

     - Parameter tempId: The temporary identifier for this entity.

     - Throws: `ResultError.error` if the creation of the transaction fails.
     - Throws: `ResultError.empty` if no `EntityBuilder` is created.

     - Returns: an `EntityBuilder` for this `InProgress`
     */
    func entityBuilder(forTempId tempId: String) throws -> EntityBuilder {
        let result = store_entity_builder_from_temp_id(self.raw, tempId).pointee
        return EntityBuilder(raw: try result.unwrap())
    }

    /**
     Get the the `Entid` of the attribute.

     - Parameter attribute: The string represeting the attribute whose `Entid` we are after.
     The string is represented as `:namespace/name`.

     - Returns: The `Entid` associated with the attribute.
     */
    func entidForAttribute(attribute: String) -> Entid {
        return Entid(store_entid_for_attribute(self.raw, attribute))
    }

    /**
     Start a query.
     - Parameter query: The string represeting the the query to be executed.

     - Returns: The `Query` representing the query that can be executed.
     */
    func query(query: String) -> Query {
        return Query(raw: store_query(self.raw, query))
    }

    /**
     Retrieve a single value of an attribute for an Entity
     - Parameter attribute: The string the attribute whose value is to be returned.
     The string is represented as `:namespace/name`.
     - Parameter entid: The `Entid` of the entity we want the value from.

     - Returns: The `TypedValue` containing the value of the attribute for the entity.
     */
    func value(forAttribute attribute: String, ofEntity entid: Entid) throws -> TypedValue? {
        let result = store_value_for_attribute(self.raw, entid, attribute).pointee
        return TypedValue(raw: try result.unwrap())
    }

    // Destroys the pointer by passing it back into Rust to be cleaned up
    override func cleanup(pointer: OpaquePointer) {
        store_destroy(pointer)
    }
}

/**
 Set up `Mentat` to provide an interface to Mentat's transaction observation
 */
extension Mentat: Observable {
     /**
     Register an `Observing` and a set of attributes to observer for transaction observation.
     The `transactionDidOccur(String: [TxChange]:)` function is called when a transaction
     occurs in the `Store` that this `Mentat` is connected to that affects the attributes that an
     `Observing` has registered for.

     - Parameter key: `String` representing an identifier for the `Observing`.
     - Parameter observer: The `Observing` to be notified when a transaction occurs.
     - Parameter attributes: An `Array` of `Strings` representing the attributes that the `Observing`
     wishes to be notified about if they are referenced in a transaction.
     */
    func register(key: String, observer: Observing, attributes: [String]) {
        let attrEntIds = attributes.map({ (kw) -> Entid in
            let entid = Entid(self.entidForAttribute(attribute: kw));
            return entid
        })

        let ptr = UnsafeMutablePointer<Entid>.allocate(capacity: attrEntIds.count)
        let entidPointer = UnsafeMutableBufferPointer(start: ptr, count: attrEntIds.count)
        var _ = entidPointer.initialize(from: attrEntIds)

        guard let firstElement = entidPointer.baseAddress else {
            return
        }
        Mentat.observers[key] = observer
        store_register_observer(self.raw, key, firstElement, Entid(attributes.count), transactionObserverCallback)

    }

    /**
     Unregister the `Observing` that was registered with the provided key such that it will no longer be called
     if a transaction occurs that affects the attributes that `Observing` was registered to observe.

     The `Observing` will need to re-register if it wants to start observing again.

     - Parameter key: `String` representing an identifier for the `Observing`.
     */
    func unregister(key: String) {
        Mentat.observers.removeValue(forKey: key)
        store_unregister_observer(self.raw, key)
    }
}


/**
 This function needs to be static as callbacks passed into Rust from Swift cannot contain state. Therefore the observers are static, as is
 the function that we pass into Rust to receive the callback.
 */
private func transactionObserverCallback(key: UnsafePointer<CChar>, reports: UnsafePointer<TxChangeList>) {
    let key = String(cString: key)
    guard let observer = Mentat.observers[key] else { return }
    DispatchQueue.global(qos: .background).async {
        observer.transactionDidOccur(key: key, reports: [TxChange]())
    }
}
