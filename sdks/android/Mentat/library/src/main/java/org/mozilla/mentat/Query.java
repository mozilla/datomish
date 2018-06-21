/* -*- Mode: Java; c-basic-offset: 4; tab-width: 20; indent-tabs-mode: nil; -*-
 * Copyright 2018 Mozilla
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use
 * this file except in compliance with the License. You may obtain a copy of the
 * License at http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software distributed
 * under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
 * CONDITIONS OF ANY KIND, either express or implied. See the License for the
 * specific language governing permissions and limitations under the License. */

package org.mozilla.mentat;

import android.util.Log;

import java.util.Date;
import java.util.UUID;

/**
 * This class allows you to construct a query, bind values to variables and run those queries against a mentat DB.
 * <p/>
 * This class cannot be created directly, but must be created through `Mentat.query(String:)`.
 * <p/>
 *  The types of values you can bind are:
 * <ul>
 * <li>{@link TypedValue}</li>
 * <li>long</li>
 * <li>Entid (as long)</li>
 * <li>Keyword (as String)</li>
 * <li>boolean</li>
 * <li>double</li>
 * <li>{@link Date}</li>
 * <li>{@link String}</li>
 * <li>{@link UUID}</li>
 * </ul>
 * <p>
 * <p/>
 * Each bound variable must have a corresponding value in the query string used to create this query.
 * <p/>
 * <pre>{@code
 * String query = "[:find ?name ?cat\n" +
 *          "        :in ?type\n" +
 *          "        :where\n" +
 *          "        [?c :community/name ?name]\n" +
 *          "        [?c :community/type ?type]\n" +
 *          "        [?c :community/category ?cat]]";
 * mentat.query(query).bindKeywordReference("?type", ":community.type/website").run(new RelResultHandler() {
 *      @Override
 *      public void handleRows(RelResult rows) {
 *          ...
 *      }
 * });
 *}</pre>
 * <p/>
 * Queries can be run and the results returned in a number of different formats. Individual result values are returned as `TypedValues` and
 * the format differences relate to the number and structure of those values. The result format is related to the format provided in the query string.
 * <p/>
 * - `Rel` - This is the default `run` function and returns a list of rows of values. Queries that wish to have `Rel` results should format their query strings:
 *
 * <pre>{@code
 * String query = "[: find ?a ?b ?c\n" +
 *          "        : where ... ]";
 * mentat.query(query).run(new RelResultHandler() {
 *      @Override
 *      public void handleRows(RelResult rows) {
 *          ...
 *      }
 * });
 *}</pre>
 * <p/>
 * - `Scalar` - This returns a single value as a result. This can be optional, as the value may not be present. Queries that wish to have `Scalar` results should format their query strings:
 *
 * <pre>{@code
 * String query = "[: find ?a .\n" +
 *          "        : where ... ]";
 * mentat.query(query).run(new ScalarResultHandler() {
 *      @Override
 *      public void handleValue(TypedValue value) {
 *          ...
 *      }
 * });
 *}</pre>
 * <p/>
 * - `Coll` - This returns a list of single values as a result.  Queries that wish to have `Coll` results should format their query strings:
 * <pre>{@code
 * String query = "[: find [?a ...]\n" +
 *          "        : where ... ]";
 * mentat.query(query).run(new ScalarResultHandler() {
 *      @Override
 *      public void handleList(CollResult list) {
 *          ...
 *      }
 * });
 *}</pre>
 * <p/>
 * - `Tuple` - This returns a single row of values.  Queries that wish to have `Tuple` results should format their query strings:
 * <pre>{@code
 * String query = "[: find [?a ?b ?c]\n" +
 *          "        : where ... ]";
 * mentat.query(query).run(new TupleResultHandler() {
 *      @Override
 *      public void handleRow(TupleResult row) {
 *          ...
 *      }
 * });
 *}</pre>
 */
public class Query extends RustObject<JNA.QueryBuilder> {

    public Query(JNA.QueryBuilder pointer) {
        super(pointer);
    }

    /**
     * Binds a long value to the provided variable name.
     * TODO: Throw an exception if the query raw pointer has been consumed.
     * @param varName   The name of the variable in the format `?name`.
     * @param value The value to be bound
     * @return  This {@link Query} such that further function can be called.
     */
    public Query bind(String varName, long value) {
        this.assertValidPointer();
        JNA.INSTANCE.query_builder_bind_long(this.validPointer(), varName, value);
        return this;
    }

    /**
     * Binds a Entid value to the provided variable name.
     * TODO: Throw an exception if the query raw pointer has been consumed.
     * @param varName   The name of the variable in the format `?name`.
     * @param value The value to be bound
     * @return  This {@link Query} such that further function can be called.
     */
    public Query bindEntidReference(String varName, long value) {
        JNA.INSTANCE.query_builder_bind_ref(this.validPointer(), varName, value);
        return this;
    }

    /**
     * Binds a String keyword value to the provided variable name.
     * TODO: Throw an exception if the query raw pointer has been consumed.
     * @param varName   The name of the variable in the format `?name`.
     * @param value The value to be bound
     * @return  This {@link Query} such that further function can be called.
     */
    public Query bindKeywordReference(String varName, String value) {
        JNA.INSTANCE.query_builder_bind_ref_kw(this.validPointer(), varName, value);
        return this;
    }

    /**
     * Binds a keyword value to the provided variable name.
     * TODO: Throw an exception if the query raw pointer has been consumed.
     * @param varName   The name of the variable in the format `?name`.
     * @param value The value to be bound
     * @return  This {@link Query} such that further function can be called.
     */
    public Query bindKeyword(String varName, String value) {
        JNA.INSTANCE.query_builder_bind_kw(this.validPointer(), varName, value);
        return this;
    }

    /**
     * Binds a boolean value to the provided variable name.
     * TODO: Throw an exception if the query raw pointer has been consumed.
     * @param varName   The name of the variable in the format `?name`.
     * @param value The value to be bound
     * @return  This {@link Query} such that further function can be called.
     */
    public Query bind(String varName, boolean value) {
        JNA.INSTANCE.query_builder_bind_boolean(this.validPointer(), varName, value ? 1 : 0);
        return this;
    }

    /**
     * Binds a double value to the provided variable name.
     * TODO: Throw an exception if the query raw pointer has been consumed.
     * @param varName   The name of the variable in the format `?name`.
     * @param value The value to be bound
     * @return  This {@link Query} such that further function can be called.
     */
    public Query bind(String varName, double value) {
        JNA.INSTANCE.query_builder_bind_double(this.validPointer(), varName, value);
        return this;
    }

    /**
     * Binds a {@link Date} value to the provided variable name.
     * TODO: Throw an exception if the query raw pointer has been consumed.
     * @param varName   The name of the variable in the format `?name`.
     * @param value The value to be bound
     * @return  This {@link Query} such that further function can be called.
     */
    public Query bind(String varName, Date value) {
        long timestamp = value.getTime() * 1000;
        JNA.INSTANCE.query_builder_bind_timestamp(this.validPointer(), varName, timestamp);
        return this;
    }

    /**
     * Binds a {@link String} value to the provided variable name.
     * TODO: Throw an exception if the query raw pointer has been consumed.
     * @param varName   The name of the variable in the format `?name`.
     * @param value The value to be bound
     * @return  This {@link Query} such that further function can be called.
     */
    public Query bind(String varName, String value) {
        JNA.INSTANCE.query_builder_bind_string(this.validPointer(), varName, value);
        return this;
    }

    /**
     * Binds a {@link UUID} value to the provided variable name.
     * TODO: Throw an exception if the query raw pointer has been consumed.
     * @param varName   The name of the variable in the format `?name`.
     * @param value The value to be bound
     * @return  This {@link Query} such that further function can be called.
     */
    public Query bind(String varName, UUID value) {
        JNA.INSTANCE.query_builder_bind_uuid(this.validPointer(), varName, getPointerForUUID(value));
        return this;
    }

    /**
     * Execute the query with the values bound associated with this {@link Query} and call the provided
     * callback function with the results as a list of rows of {@link TypedValue}s.
     * TODO: Throw an exception if the query raw pointer has been consumed or the query fails to execute
     * @param handler   the handler to call with the results of this query
     */
    public void run(final RelResultHandler handler) {
        RustError.ByReference error = new RustError.ByReference();
        JNA.RelResult relResult = JNA.INSTANCE.query_builder_execute(this.consumePointer(), error);
        if (error.isFailure()) {
            Log.e("Query", error.consumeErrorMessage());
            return;
        }
        handler.handleRows(new RelResult(relResult));
    }

    /**
     * Execute the query with the values bound associated with this {@link Query} and call the provided
     * callback function with the results with the result as a single {@link TypedValue}.
     * TODO: Throw an exception if the query raw pointer has been consumed or the query fails to execute
     * @param handler   the handler to call with the results of this query
     */
    public void run(final ScalarResultHandler handler) {
        RustError.ByReference error = new RustError.ByReference();
        JNA.TypedValue valOrNull = JNA.INSTANCE.query_builder_execute_scalar(consumePointer(), error);

        if (error.isFailure()) {
            Log.e("Query", error.consumeErrorMessage());
            return;
        }

        if (valOrNull != null) {
            handler.handleValue(new TypedValue(valOrNull));
        } else {
            handler.handleValue(null);
        }
    }

    /**
     * Execute the query with the values bound associated with this {@link Query} and call the provided
     * callback function with the results with the result as a list of single {@link TypedValue}s.
     * TODO: Throw an exception if the query raw pointer has been consumed or the query fails to execute
     * @param handler   the handler to call with the results of this query
     */
    public void run(final CollResultHandler handler) {
        RustError.ByReference error = new RustError.ByReference();
        JNA.TypedValueList collResult = JNA.INSTANCE.query_builder_execute_coll(this.consumePointer(), error);

        if (error.isFailure()) {
            Log.e("Query", error.consumeErrorMessage());
            return;
        }
        handler.handleList(new CollResult(collResult));
    }

    /**
     * Execute the query with the values bound associated with this {@link Query} and call the provided
     * callback function with the results with the result as a list of single {@link TypedValue}s.
     * TODO: Throw an exception if the query raw pointer has been consumed or the query fails to execute
     * @param handler   the handler to call with the results of this query
     */
    public void run(final TupleResultHandler handler) {
        RustError.ByReference error = new RustError.ByReference();
        JNA.TypedValueList tuple = JNA.INSTANCE.query_builder_execute_tuple(this.consumePointer(), error);

        if (error.isFailure()) {
            Log.e("Query", error.consumeErrorMessage());
            return;
        }

        if (tuple != null) {
            handler.handleRow(new TupleResult(tuple));
        } else {
            handler.handleRow(null);
        }
    }

    @Override
    protected void destroyPointer(JNA.QueryBuilder p) {
        JNA.INSTANCE.query_builder_destroy(p);
    }
}
