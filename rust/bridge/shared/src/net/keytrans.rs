//
// Copyright 2024 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use std::time::SystemTime;

use libsignal_bridge_macros::{bridge_fn, bridge_io};
use libsignal_bridge_types::net::chat::UnauthChat;
pub use libsignal_bridge_types::net::{Environment, TokioAsyncContext};
use libsignal_bridge_types::support::AsType;
use libsignal_core::{Aci, E164};
use libsignal_keytrans::{
    AccountData, KeyTransparency, LocalStateUpdate, StoredAccountData, StoredTreeHead,
};
use libsignal_net::keytrans::{Error, Kt, SearchKey, SearchResult, UsernameHash};
use libsignal_protocol::PublicKey;
use prost::{DecodeError, Message};

use crate::support::*;
use crate::*;

#[bridge_fn(node = false, ffi = false)]
fn KeyTransparency_AciSearchKey(aci: Aci) -> Vec<u8> {
    aci.as_search_key()
}

#[bridge_fn(node = false, ffi = false)]
fn KeyTransparency_E164SearchKey(e164: E164) -> Vec<u8> {
    e164.as_search_key()
}

#[bridge_fn(node = false, ffi = false)]
fn KeyTransparency_UsernameHashSearchKey(hash: &[u8]) -> Vec<u8> {
    UsernameHash::from_slice(hash).as_search_key()
}

bridge_handle_fns!(SearchResult, clone = false, ffi = false, node = false);

#[bridge_fn(node = false, ffi = false)]
fn SearchResult_GetAciIdentityKey(res: &SearchResult) -> PublicKey {
    *res.aci_identity_key.public_key()
}

#[bridge_fn(node = false, ffi = false)]
fn SearchResult_GetAciForE164(res: &SearchResult) -> Option<Aci> {
    res.aci_for_e164
}

#[bridge_fn(node = false, ffi = false)]
fn SearchResult_GetAciForUsernameHash(res: &SearchResult) -> Option<Aci> {
    res.aci_for_username_hash
}

#[bridge_fn(node = false, ffi = false)]
fn SearchResult_GetTimestamp(res: &SearchResult) -> u64 {
    res.timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("valid timestamp")
        .as_millis()
        .try_into()
        .expect("in u64 range")
}

#[bridge_fn(node = false, ffi = false)]
fn SearchResult_GetAccountData(res: &SearchResult) -> Vec<u8> {
    res.account_data.encode_to_vec()
}

#[cfg(feature = "jni")]
fn try_decode<B, T>(bytes: B) -> Result<T, DecodeError>
where
    B: AsRef<[u8]>,
    T: Message + Default,
{
    T::decode(bytes.as_ref())
}

#[bridge_io(TokioAsyncContext, node = false, ffi = false)]
#[allow(clippy::too_many_arguments)]
async fn KeyTransparency_Search(
    // TODO: it is currently possible to pass an env that does not match chat
    environment: AsType<Environment, u8>,
    chat: &UnauthChat,
    aci: Aci,
    aci_identity_key: &PublicKey,
    e164: Option<E164>,
    unidentified_access_key: Option<Box<[u8]>>,
    username_hash: Option<Box<[u8]>>,
    account_data: Option<Box<[u8]>>,
    last_distinguished_tree_head: Box<[u8]>,
) -> Result<SearchResult, Error> {
    let username_hash = username_hash.map(UsernameHash::from);
    let config = environment
        .into_inner()
        .env()
        .keytrans_config
        .expect("keytrans config must be set")
        .into();
    let kt = Kt {
        inner: KeyTransparency { config },
        chat: &chat.service.0,
        config: Default::default(),
    };

    let e164_pair = match (e164, unidentified_access_key) {
        (None, None) => None,
        (Some(e164), Some(uak)) => Some((e164, uak.into_vec())),
        // technically harmless, but still invalid
        (None, Some(_uak)) => {
            return Err(Error::InvalidRequest(
                "Unidentified access key without an E164",
            ))
        }
        (Some(_e164), None) => {
            return Err(Error::InvalidRequest(
                "E164 without unidentified access key",
            ))
        }
    };

    let account_data = account_data
        .map(|bytes| {
            let stored: StoredAccountData = try_decode(bytes)?;
            AccountData::try_from(stored).map_err(Error::from)
        })
        .transpose()?;

    let last_distinguished_tree_head =
        try_decode(last_distinguished_tree_head).map(|stored: StoredTreeHead| stored.tree_head)?;
    let distinguished_tree_head_size = last_distinguished_tree_head
        .map(|head| head.tree_size)
        .ok_or(Error::InvalidRequest("distinguished tree head is missing"))?;

    let result = kt
        .search(
            &aci,
            aci_identity_key,
            e164_pair,
            username_hash,
            account_data,
            distinguished_tree_head_size,
        )
        .await?;
    Ok(result)
}

#[bridge_io(TokioAsyncContext, node = false, ffi = false)]
async fn KeyTransparency_Distinguished(
    // TODO: it is currently possible to pass an env that does not match chat
    environment: AsType<Environment, u8>,
    chat: &UnauthChat,
    last_distinguished_tree_head: Option<Box<[u8]>>,
) -> Result<Vec<u8>, Error> {
    let config = environment
        .into_inner()
        .env()
        .keytrans_config
        .expect("keytrans config must be set")
        .into();
    let kt = Kt {
        inner: KeyTransparency { config },
        chat: &chat.service.0,
        config: Default::default(),
    };

    let known_distinguished = last_distinguished_tree_head
        .map(try_decode)
        .transpose()?
        .and_then(|stored: StoredTreeHead| stored.into_last_tree_head());
    let LocalStateUpdate {
        tree_head,
        tree_root,
        monitoring_data: _,
    } = kt.distinguished(known_distinguished).await?;
    let updated_distinguished = StoredTreeHead::from((tree_head, tree_root));
    let serialized = updated_distinguished.encode_to_vec();
    Ok(serialized)
}
