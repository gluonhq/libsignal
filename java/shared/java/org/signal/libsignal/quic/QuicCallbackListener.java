//
// Copyright 2023 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

package org.signal.libsignal.quic;

public interface QuicCallbackListener {

  void onData(byte[] data);
}
