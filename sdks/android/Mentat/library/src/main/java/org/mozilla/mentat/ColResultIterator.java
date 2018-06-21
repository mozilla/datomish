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

import com.sun.jna.Pointer;

/**
 * Iterator for a {@link CollResult}
 */
public class ColResultIterator extends RustIterator<JNA.TypedValueListIter, JNA.TypedValue, TypedValue> {

    ColResultIterator(JNA.TypedValueListIter iterator) {
        super(iterator);
    }

    @Override
    protected JNA.TypedValue advanceIterator() {
        return JNA.INSTANCE.typed_value_list_iter_next(this.validPointer());
    }

    @Override
    protected TypedValue constructItem(JNA.TypedValue p) {
        return new TypedValue(p);
    }

    @Override
    protected void destroyPointer(JNA.TypedValueListIter p) {
        JNA.INSTANCE.typed_value_list_iter_destroy(p);
    }
}
