//
// Copyright 2023 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use ::signal_grpc::GrpcClient;
use ::signal_grpc::Result;
use libsignal_bridge_macros::*;

use crate::support::*;
use crate::*;

use std::collections::HashMap;

#[cfg(all(feature = "jni"))]
pub struct GrpcHeaders(pub HashMap<String, Vec<String>>);

#[bridge_fn(ffi = false, node = false)]
pub fn Grpc_SendMessage(method: String, url_fragment: String, body: &[u8], headers: GrpcHeaders) -> Result<Vec<u8>> {
    GrpcClient::new()?.send_message(method, url_fragment, body, headers.0)
}
