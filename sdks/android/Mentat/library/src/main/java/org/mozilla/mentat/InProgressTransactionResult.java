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

import com.sun.jna.Pointer;
import com.sun.jna.Structure;

import java.util.Arrays;
import java.util.List;

public class InProgressTransactionResult extends Structure {
    public static class ByReference extends InProgressTransactionResult implements Structure.ByReference {
    }

    public static class ByValue extends InProgressTransactionResult implements Structure.ByValue {
    }

    public JNA.InProgress inProgress;
    public JNA.TxReport txReport;
    public RustError error;

    @Override
    protected List<String> getFieldOrder() {
        return Arrays.asList("inProgress", "txReport", "error");
    }

    public InProgress getInProgress() {
        if (this.inProgress == null) {
            throw new NullPointerException("Already consumed InProgress");
        }
        InProgress ip = new InProgress(this.inProgress);
        this.inProgress = null;
        return ip;

    }

    public TxReport getReport() {
        if (this.error.isFailure()) {
            Log.e("InProgressTransactRes", this.error.consumeErrorMessage());
            return null;
        }
        if (this.txReport == null) {
            throw new NullPointerException("Already consumed TxReport");
        }
        JNA.TxReport report = this.txReport;
        this.txReport = null;
        return new TxReport(report);
    }

    @Override
    protected void finalize() {
        if (this.txReport != null) {
            JNA.INSTANCE.tx_report_destroy(this.txReport);
            this.txReport = null;
        }
        if (this.inProgress != null) {
            JNA.INSTANCE.in_progress_destroy(this.inProgress);
            this.inProgress = null;
        }
    }
}
