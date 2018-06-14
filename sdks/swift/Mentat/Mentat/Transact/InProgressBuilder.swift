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

/**
 This class wraps a raw pointer that points to a Rust `InProgressBuilder` object.

 `InProgressBuilder` provides a programmatic interface to performing assertions for entities.
 It provides functions for adding and retracting values for attributes for an entity within
 an in progress transaction.

 The `transact` function will transact the assertions that have been added to the `InProgressBuilder`
 and pass back the `TxReport` that was generated by this transact and the `InProgress` that was
 used to perform the transact. This enables you to perform further transacts on the same `InProgress`
 before committing.

 ```
 let aEntid = txReport.entid(forTempId: "a")
 let bEntid = txReport.entid(forTempId: "b")
 do {
     let builder = try mentat.entityBuilder()
     try builder.add(entid: bEntid, keyword: ":foo/boolean", boolean: true)
     try builder.add(entid: aEntid, keyword: ":foo/instant", date: newDate)
     let (inProgress, report) = try builder.transact()
     try inProgress.transact(transaction: "[[:db/add \(aEntid) :foo/long 22]]")
     try inProgress.commit()
     ...
 } catch {
    ...
 }
 ```

 The `commit` function will transact and commit the assertions that have been added to the `EntityBuilder`.
 It will consume the `InProgress` used to perform the transact. It returns the `TxReport` generated by
 the transact. After calling `commit`, a new transaction must be started by calling `Mentat.beginTransaction()`
 in order to perform further actions.

 ```
 let aEntid = txReport.entid(forTempId: "a")
 let bEntid = txReport.entid(forTempId: "b")
 do {
     let builder = try mentat.entityBuilder(forEntid: aEntid)
     try builder.add(entid: bEntid, keyword: ":foo/boolean", boolean: true)
     try builder.add(entid: aEntid, keyword: ":foo/instant", date: newDate)
     let report = try builder.commit()
    ...
 } catch {
    ...
 }
 ```
 */
open class InProgressBuilder: OptionalRustObject {

    /**
     Asserts the value of attribute `keyword` to be the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be asserted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/long`.
     */
    open func add(entid: Entid, keyword: String, long value: Int64) throws {
        try in_progress_builder_add_long(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Asserts the value of attribute `keyword` to be the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be asserted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/ref`.
     */
    open func add(entid: Entid, keyword: String, reference value: Entid) throws {
        try in_progress_builder_add_ref(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Asserts the value of attribute `keyword` to be the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be asserted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/keyword`.
     */
    open func add(entid: Entid, keyword: String, keyword value: String) throws {
        try in_progress_builder_add_keyword(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Asserts the value of attribute `keyword` to be the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be asserted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/boolean`.
     */
    open func add(entid: Entid, keyword: String, boolean value: Bool) throws {
        try in_progress_builder_add_boolean(try self.validPointer(), entid, keyword, value ? 1 : 0).tryUnwrap()
    }

    /**
     Asserts the value of attribute `keyword` to be the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be asserted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/double`.
     */
    open func add(entid: Entid, keyword: String, double value: Double) throws {
        try in_progress_builder_add_double(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Asserts the value of attribute `keyword` to be the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be asserted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/instant`.
     */
    open func add(entid: Entid, keyword: String, date value: Date) throws {
        try in_progress_builder_add_timestamp(try self.validPointer(), entid, keyword, value.toMicroseconds()).tryUnwrap()
    }

    /**
     Asserts the value of attribute `keyword` to be the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be asserted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/string`.
     */
    open func add(entid: Entid, keyword: String, string value: String) throws {
        try in_progress_builder_add_string(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Asserts the value of attribute `keyword` to be the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be asserted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/uuid`.
     */
    open func add(entid: Entid, keyword: String, uuid value: UUID) throws {
        var rawUuid = value.uuid
        let _ = try withUnsafePointer(to: &rawUuid) { uuidPtr in
            try in_progress_builder_add_uuid(try self.validPointer(), entid, keyword, uuidPtr).tryUnwrap()
        }
    }

    /**
     Retracts the value of attribute `keyword` from the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be retracted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/long`.
     */
    open func retract(entid: Entid, keyword: String, long value: Int64) throws {
        try in_progress_builder_retract_long(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Retracts the value of attribute `keyword` from the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be retracted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/ref`.
     */
    open func retract(entid: Entid, keyword: String, reference value: Entid) throws {
        try in_progress_builder_retract_ref(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Retracts the value of attribute `keyword` from the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be retracted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/keyword`.
     */
    open func retract(entid: Entid, keyword: String, keyword value: String) throws {
        try in_progress_builder_retract_keyword(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Retracts the value of attribute `keyword` from the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be retracted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/boolean`.
     */
    open func retract(entid: Entid, keyword: String, boolean value: Bool) throws {
        try in_progress_builder_retract_boolean(try self.validPointer(), entid, keyword, value ? 1 : 0).tryUnwrap()
    }

    /**
     Retracts the value of attribute `keyword` from the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be retracted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/double`.
     */
    open func retract(entid: Entid, keyword: String, double value: Double) throws {
        try in_progress_builder_retract_double(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Retracts the value of attribute `keyword` from the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be retracted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/instant`.
     */
    open func retract(entid: Entid, keyword: String, date value: Date) throws {
        try in_progress_builder_retract_timestamp(try self.validPointer(), entid, keyword, value.toMicroseconds()).tryUnwrap()
    }

    /**
     Retracts the value of attribute `keyword` from the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be retracted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/string`.
     */
    open func retract(entid: Entid, keyword: String, string value: String) throws {
        try in_progress_builder_retract_string(try self.validPointer(), entid, keyword, value).tryUnwrap()
    }

    /**
     Retracts the value of attribute `keyword` from the provided `value` for entity `entid`.

     - Parameter entid: The `Entid` of the entity to be touched.
     - Parameter keyword: The name of the attribute in the format `:namespace/name`.
     - Parameter value: The value to be retracted

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if the attribute is not present in the schema or the attribute value type
     is not `:db.type/uuid`.
     */
    open func retract(entid: Entid, keyword: String, uuid value: UUID) throws {
        var rawUuid = value.uuid
        let _ = try withUnsafePointer(to: &rawUuid) { uuidPtr in
            try in_progress_builder_retract_uuid(try self.validPointer(), entid, keyword, uuidPtr).tryUnwrap()
        }
    }

    /**
     Transacts the added assertions. This consumes the pointer associated with this `InProgressBuilder`
     such that no further assertions can be added after the `transact` has completed. To perform
     further assertions, use the `InProgress` returned from this function.

     This does not commit the transaction. In order to do so, `commit` can be called on the `InProgress` returned
     from this function.

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if an error occured during the execution of the transact.

     - Returns: The current `InProgress` and the `TxReport` generated by the transact.
     */
    open func transact() throws -> (InProgress, TxReport?) {
        defer {
            self.raw = nil
        }
        let result = in_progress_builder_transact(try self.validPointer())
        defer {
            destroy(result);
        }
        let inProgress = InProgress(raw: result.pointee.inProgress)
        guard let report = try result.pointee.result.tryUnwrap() else {
            return (inProgress, nil)
        }
        return (inProgress, TxReport(raw: report))
    }

    /**
     Transacts the added assertions and commits. This consumes the pointer associated with this `InProgressBuilder`
     and the associated `InProgress` such that no further assertions can be added after the `commit` has completed.
     To perform further assertions, a new `InProgress` or `InProgressBuilder` should be created.

     - Throws: `PointerError.pointerConsumed` if the underlying raw pointer has already consumed, which will occur if the builder
     has already been transacted or committed.
     - Throws: `ResultError.error` if an error occured during the execution of the transact.

     - Returns: The `TxReport` generated by the transact.
     */
    open func commit() throws -> TxReport {
        defer {
            self.raw = nil
        }
        return TxReport(raw: try in_progress_builder_commit(try self.validPointer()).unwrap())
    }

    override open func cleanup(pointer: OpaquePointer) {
        in_progress_builder_destroy(pointer)
    }
}
