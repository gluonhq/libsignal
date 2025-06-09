//
// Copyright 2024 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//
use std::time::Duration;

use futures_util::future::BoxFuture;
use libsignal_net::chat;
use libsignal_net::keytrans::UnauthenticatedChat;

use crate::net::chat::BridgeChatConnection as _;

impl UnauthenticatedChat for crate::net::chat::UnauthenticatedChatConnection {
    fn send_unauthenticated(
        &self,
        request: chat::Request,
        timeout: Duration,
    ) -> BoxFuture<'_, Result<chat::Response, chat::SendError>> {
        Box::pin(self.send(request, timeout))
    }
}
