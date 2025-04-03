use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use base64::Engine as _;
use http::uri::PathAndQuery;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use libsignal_core::{Aci, Pni};
use libsignal_net_infra::errors::{LogSafeDisplay, RetryLater};
use libsignal_net_infra::{extract_retry_later, AsHttpHeader as _};
use libsignal_protocol::{GenericSignedPreKey, KyberPreKeyRecord, PublicKey, SignedPreKeyRecord};
use serde_with::{serde_as, skip_serializing_none, DurationSeconds, FromInto};
use uuid::Uuid;

use crate::auth::Auth;
use crate::registration::SessionId;

pub(super) const CONTENT_TYPE_JSON: (HeaderName, HeaderValue) = (
    http::header::CONTENT_TYPE,
    HeaderValue::from_static("application/json"),
);

#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSession {
    pub number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_token_type: Option<PushTokenType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mnc: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSession {}

#[serde_as]
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
#[serde(rename_all = "camelCase", default)]
pub struct RegistrationSession {
    pub allowed_to_request_code: bool,
    pub verified: bool,
    #[serde_as(as = "Option<DurationSeconds>")]
    pub next_sms: Option<Duration>,
    #[serde_as(as = "Option<DurationSeconds>")]
    pub next_call: Option<Duration>,
    #[serde_as(as = "Option<DurationSeconds>")]
    pub next_verification_attempt: Option<Duration>,
    pub requested_information: HashSet<RequestedInformation>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Deserialize, strum::AsRefStr)]
#[strum(serialize_all = "camelCase")]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum RequestedInformation {
    PushChallenge,
    Captcha,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, strum::EnumString)]
#[strum(serialize_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub enum PushTokenType {
    Apn,
    Fcm,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, strum::EnumString)]
#[strum(serialize_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub enum VerificationTransport {
    Sms,
    Voice,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VerificationCodeNotDeliverable {
    // This could be a stronger type but we don't need it to be in libsignal and
    // the additional flexibility could be useful if the server adds more
    // "reason" values.
    pub reason: String,
    pub permanent_failure: bool,
}

/// The subset of account attributes that don't need any additional validation.
#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProvidedAccountAttributes<'a> {
    #[serde_as(as = "Base64Padded")]
    pub recovery_password: &'a [u8],
    /// Generated ID associated with a user's ACI.
    pub registration_id: u16,
    /// Generated ID associated with a user's PNI.
    pub pni_registration_id: u16,
    /// Protobuf-encoded device name.
    #[serde_as(as = "Option<Base64Padded>")]
    pub name: Option<&'a [u8]>,
    pub registration_lock: Option<&'a str>,
    /// Generated from the user's profile key.
    pub unidentified_access_key: Option<&'a [u8; zkgroup::ACCESS_KEY_LEN]>,
    /// Whether the user allows sealed sender messages to come from arbitrary senders.
    pub unrestricted_unidentified_access: bool,
    #[serde_as(as = "MappedToTrue")]
    pub capabilities: HashSet<&'a str>,
    pub discoverable_by_phone_number: bool,
    pub each_registration_id_valid: Option<bool>,
}

#[serde_as]
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAccountResponse {
    #[serde_as(as = "Option<FromInto<Uuid>>")]
    #[serde(rename = "uuid")]
    pub aci: Option<Aci>,
    pub number: Option<String>,
    #[serde_as(as = "Option<FromInto<Uuid>>")]
    pub pni: Option<Pni>,
    #[serde_as(as = "Option<Base64Padded>")]
    pub username_hash: Option<Box<[u8]>>,
}

#[serde_as]
#[derive(Copy, Clone, Debug, Eq, PartialEq, serde::Serialize, strum::EnumTryAs)]
#[serde(rename_all = "camelCase")]
pub enum SessionValidation<'a> {
    SessionId(&'a SessionId),
    RecoveryPassword(#[serde_as(as = "Base64Padded")] &'a [u8]),
}

/// Pair of values where one is for an ACI and the other a PNI.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ForServiceIds<T> {
    pub aci: T,
    pub pni: T,
}

/// How a device wants to be notified of messages when offline.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum NewMessageNotification<'a> {
    /// Use the provided APN ID to receive push notifications.
    Apn(&'a str),
    /// Use the provided GCM/FCM ID to receive push notifications.
    Gcm(&'a str),
    /// The device will poll on its own.
    WillFetchMessages,
}

#[skip_serializing_none]
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateRegistrationSession<'a> {
    pub(super) captcha: Option<&'a str>,
    pub(super) push_token: Option<&'a str>,
    pub(crate) push_token_type: Option<PushTokenType>,
    pub(crate) push_challenge: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RequestVerificationCode<'a> {
    pub(super) transport: VerificationTransport,
    pub(super) client: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SubmitVerificationCode<'a> {
    pub(super) code: &'a str,
}

pub(super) struct RegistrationRequest<'s, R> {
    pub(super) session_id: &'s SessionId,
    pub(super) request: R,
}

pub(super) struct AccountKeys<'a> {
    identity_key: &'a PublicKey,
    signed_pre_key: &'a SignedPreKeyRecord,
    pq_last_resort_pre_key: &'a KyberPreKeyRecord,
}

#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountAttributes<'a> {
    fetches_messages: bool,
    #[serde(flatten)]
    account_attributes: ProvidedAccountAttributes<'a>,
}

/// Errors that arise from a response to a received request.
///
/// This doesn't include timeouts, since the request was known to be received
/// and the server sent a response.
#[derive(Debug, thiserror::Error, displaydoc::Display)]
pub(super) enum ResponseError {
    /// {0}
    RetryLater(RetryLater),
    /// the request did not pass server validation
    InvalidRequest,
    /// unexpected content-type {0:?}
    UnexpectedContentType(Option<HeaderValue>),
    /// unexpected response status {status}
    UnrecognizedStatus {
        status: StatusCode,
        response_headers: HeaderMap,
        response_body: Option<Box<[u8]>>,
    },
    /// response had no body
    MissingBody,
    /// response body was not valid JSON
    InvalidJson,
    /// response body didn't match the schema
    UnexpectedData,
}
impl LogSafeDisplay for ResponseError {}

#[derive(Debug, Default, PartialEq, serde::Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
#[serde(rename_all = "camelCase")]
pub(super) struct RegistrationResponse {
    #[serde(rename = "id")]
    pub(super) session_id: String,
    #[serde(flatten)]
    pub(super) session: RegistrationSession,
}

impl VerificationCodeNotDeliverable {
    pub(crate) fn from_response(
        response_headers: &HeaderMap,
        response_body: &[u8],
    ) -> Option<Self> {
        if response_headers.get(CONTENT_TYPE_JSON.0) != Some(&CONTENT_TYPE_JSON.1) {
            return None;
        }

        serde_json::from_slice(response_body).ok()
    }
}

/// A value that can be sent to the server as part of a REST request.
pub(super) trait Request {
    /// The HTTP [`Method`] to send the request with
    const METHOD: Method;

    /// The HTTP path to use when sending the request.
    fn request_path(session_id: &SessionId) -> PathAndQuery;

    /// The serialized JSON for the request body, if any.
    fn into_json_body(self) -> Option<Box<[u8]>>;
}

impl Request for GetSession {
    const METHOD: Method = Method::GET;
    fn request_path(session_id: &SessionId) -> PathAndQuery {
        format!(
            "{VERIFICATION_SESSION_PATH_PREFIX}/{}",
            session_id.as_url_path_segment()
        )
        .parse()
        .unwrap()
    }
    fn into_json_body(self) -> Option<Box<[u8]>> {
        None
    }
}

impl Request for UpdateRegistrationSession<'_> {
    const METHOD: Method = Method::PATCH;
    fn request_path(session_id: &SessionId) -> PathAndQuery {
        GetSession::request_path(session_id)
    }
    fn into_json_body(self) -> Option<Box<[u8]>> {
        Some(
            serde_json::to_vec(&self)
                .expect("no maps")
                .into_boxed_slice(),
        )
    }
}

impl Request for RequestVerificationCode<'_> {
    const METHOD: Method = Method::POST;
    fn request_path(session_id: &SessionId) -> PathAndQuery {
        format!(
            "{VERIFICATION_SESSION_PATH_PREFIX}/{}/code",
            session_id.as_url_path_segment()
        )
        .parse()
        .unwrap()
    }
    fn into_json_body(self) -> Option<Box<[u8]>> {
        Some(
            serde_json::to_vec(&self)
                .expect("no maps")
                .into_boxed_slice(),
        )
    }
}

impl Request for SubmitVerificationCode<'_> {
    const METHOD: Method = Method::PUT;
    fn request_path(session_id: &SessionId) -> PathAndQuery {
        RequestVerificationCode::request_path(session_id)
    }
    fn into_json_body(self) -> Option<Box<[u8]>> {
        Some(
            serde_json::to_vec(&self)
                .expect("no maps")
                .into_boxed_slice(),
        )
    }
}

#[cfg(test)]
impl<T> ForServiceIds<T> {
    fn generate(mut f: impl FnMut(libsignal_core::ServiceIdKind) -> T) -> Self {
        ForServiceIds {
            aci: f(libsignal_core::ServiceIdKind::Aci),
            pni: f(libsignal_core::ServiceIdKind::Pni),
        }
    }
}

/// Marker type to indicate that device transfer is being intentionally skipped.
///
/// This is usually used as `Option<SkipDeviceTransfer>` in place of a boolean
/// value.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SkipDeviceTransfer;

impl crate::chat::Request {
    #[allow(unused)]
    pub(super) fn register_account(
        session_id: Option<&SessionId>,
        message_notification: NewMessageNotification<'_>,
        account_attributes: ProvidedAccountAttributes<'_>,
        device_transfer: Option<SkipDeviceTransfer>,
        keys: ForServiceIds<AccountKeys<'_>>,
        account_password: &[u8],
        number: &str,
    ) -> Self {
        #[serde_as]
        #[skip_serializing_none]
        #[derive(Debug, serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct RegisterAccount<'a> {
            #[serde(flatten)]
            session_validation: SessionValidation<'a>,
            account_attributes: AccountAttributes<'a>,
            skip_device_transfer: bool,
            #[serde_as(as = "FromInto<PublicKeyBytes>")]
            aci_identity_key: &'a PublicKey,
            #[serde_as(as = "FromInto<PublicKeyBytes>")]
            pni_identity_key: &'a PublicKey,
            #[serde_as(as = "FromInto<SignedPrekeyBody>")]
            aci_signed_pre_key: &'a SignedPreKeyRecord,
            #[serde_as(as = "FromInto<SignedPrekeyBody>")]
            pni_signed_pre_key: &'a SignedPreKeyRecord,
            #[serde_as(as = "FromInto<SignedPrekeyBody>")]
            aci_pq_last_resort_pre_key: &'a KyberPreKeyRecord,
            #[serde_as(as = "FromInto<SignedPrekeyBody>")]
            pni_pq_last_resort_pre_key: &'a KyberPreKeyRecord,
            // Intentionally not #[serde(flatten)]-ed
            push_token: Option<PushToken<'a>>,
        }

        #[derive(Debug, serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        enum PushToken<'a> {
            ApnRegistrationId(&'a str),
            GcmRegistrationId(&'a str),
        }

        let (fetches_messages, push_token) = match message_notification {
            NewMessageNotification::Apn(apn) => (false, Some(PushToken::ApnRegistrationId(apn))),
            NewMessageNotification::Gcm(gcm) => (false, Some(PushToken::GcmRegistrationId(gcm))),
            NewMessageNotification::WillFetchMessages => (true, None),
        };

        let session_validation = session_id.map(SessionValidation::SessionId).unwrap_or(
            SessionValidation::RecoveryPassword(account_attributes.recovery_password),
        );

        let register_account = RegisterAccount {
            session_validation,
            account_attributes: AccountAttributes {
                account_attributes,
                fetches_messages,
            },
            skip_device_transfer: device_transfer.is_some_and(|SkipDeviceTransfer| true),
            aci_identity_key: keys.aci.identity_key,
            pni_identity_key: keys.pni.identity_key,
            aci_signed_pre_key: keys.aci.signed_pre_key,
            pni_signed_pre_key: keys.pni.signed_pre_key,
            aci_pq_last_resort_pre_key: keys.aci.pq_last_resort_pre_key,
            pni_pq_last_resort_pre_key: keys.pni.pq_last_resort_pre_key,
            push_token,
        };

        let body = Some(
            serde_json::to_vec(&register_account)
                .expect("no maps")
                .into_boxed_slice(),
        );

        Self {
            method: Method::POST,
            headers: HeaderMap::from_iter([
                CONTENT_TYPE_JSON,
                Auth {
                    username: number,
                    password: &base64::prelude::BASE64_STANDARD_NO_PAD.encode(account_password),
                }
                .as_header(),
            ]),
            path: PathAndQuery::from_static("/v1/registration"),
            body,
        }
    }
}

impl crate::chat::Response {
    /// Interpret `self` as a registration request response.
    pub(super) fn try_into_response<R>(self) -> Result<R, ResponseError>
    where
        R: for<'a> serde::Deserialize<'a>,
    {
        let Self {
            status,
            message: _,
            body,
            headers,
        } = self;
        if !status.is_success() {
            if status.as_u16() == 429 {
                if let Some(retry_later) = extract_retry_later(&headers) {
                    return Err(ResponseError::RetryLater(retry_later));
                }
            }
            if status.as_u16() == 422 {
                return Err(ResponseError::InvalidRequest);
            }
            log::debug!(
                "got unsuccessful response with {status}: {:?}",
                DebugAsStrOrBytes(body.as_deref().unwrap_or_default())
            );
            return Err(ResponseError::UnrecognizedStatus {
                status,
                response_headers: headers,
                response_body: body,
            });
        }
        let content_type = headers.get(http::header::CONTENT_TYPE);
        if content_type != Some(&HeaderValue::from_static("application/json")) {
            return Err(ResponseError::UnexpectedContentType(content_type.cloned()));
        }

        let body = body.ok_or(ResponseError::MissingBody)?;
        serde_json::from_slice(&body).map_err(|e| match e.classify() {
            serde_json::error::Category::Data => ResponseError::UnexpectedData,
            serde_json::error::Category::Syntax
            | serde_json::error::Category::Io
            | serde_json::error::Category::Eof => ResponseError::InvalidJson,
        })
    }
}

struct DebugAsStrOrBytes<'b>(&'b [u8]);
impl std::fmt::Debug for DebugAsStrOrBytes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match std::str::from_utf8(self.0) {
            Ok(s) => s.fmt(f),
            Err(_) => hex::encode(self.0).fmt(f),
        }
    }
}

const VERIFICATION_SESSION_PATH_PREFIX: &str = "/v1/verification/session";

impl From<CreateSession> for crate::chat::Request {
    fn from(value: CreateSession) -> Self {
        let body = serde_json::to_vec(&value)
            .expect("no maps")
            .into_boxed_slice();
        Self {
            method: Method::POST,
            headers: HeaderMap::from_iter([CONTENT_TYPE_JSON]),
            path: PathAndQuery::from_static(VERIFICATION_SESSION_PATH_PREFIX),
            body: Some(body),
        }
    }
}

impl<'s, R: Request> From<RegistrationRequest<'s, R>> for crate::chat::Request {
    fn from(value: RegistrationRequest<'s, R>) -> Self {
        let RegistrationRequest {
            session_id,
            request,
        } = value;

        let path = R::request_path(session_id);
        let body = request.into_json_body();
        let headers = HeaderMap::from_iter(body.is_some().then_some(CONTENT_TYPE_JSON));

        Self {
            method: R::METHOD,
            headers,
            path,
            body,
        }
    }
}

type Base64Padded =
    serde_with::base64::Base64<serde_with::base64::Standard, serde_with::formats::Padded>;

#[serde_as]
#[derive(serde::Serialize)]
#[serde(transparent)]
struct PublicKeyBytes(#[serde_as(as = "Base64Padded")] Box<[u8]>);

impl From<&PublicKey> for PublicKeyBytes {
    fn from(value: &PublicKey) -> Self {
        Self(value.serialize())
    }
}

#[serde_as]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SignedPrekeyBody<'a> {
    key_id: u32,
    #[serde_as(as = "Base64Padded")]
    public_key: &'a [u8],
    #[serde_as(as = "Base64Padded")]
    signature: &'a [u8],
}

impl<'a, T: GenericSignedPreKey> From<&'a T> for SignedPrekeyBody<'a> {
    fn from(record: &'a T) -> Self {
        let storage = record.get_storage();
        SignedPrekeyBody {
            key_id: storage.id,
            public_key: &storage.public_key,
            signature: &storage.signature,
        }
    }
}

struct MappedToTrue;

impl<T> serde_with::SerializeAs<HashSet<T>> for MappedToTrue
where
    T: serde::Serialize,
{
    fn serialize_as<S>(source: &HashSet<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(source.iter().map(|name| (name, true)))
    }
}

impl TryFrom<String> for VerificationTransport {
    type Error = <Self as FromStr>::Err;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        FromStr::from_str(&value)
    }
}

#[cfg(test)]
impl RegistrationResponse {
    pub(super) fn into_websocket_response(
        self,
        ws_request_id: u64,
    ) -> crate::proto::chat_websocket::WebSocketResponseMessage {
        crate::proto::chat_websocket::WebSocketResponseMessage {
            id: Some(ws_request_id),
            status: Some(http::StatusCode::OK.as_u16().into()),
            message: Some("OK".to_string()),
            headers: vec!["content-type: application/json".to_owned()],
            body: Some(serde_json::to_vec(&self).unwrap()),
        }
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr as _;
    use std::sync::LazyLock;

    use libsignal_protocol::KeyPair;
    use rand::SeedableRng as _;
    use serde_json::json;

    use super::*;
    use crate::chat::{Request as ChatRequest, Response as ChatResponse};

    #[test]
    fn registration_get_session_request_as_chat_request() {
        let request: ChatRequest = RegistrationRequest {
            session_id: &SessionId::from_str("aaabbbcccdddeee").unwrap(),
            request: GetSession {},
        }
        .into();

        assert_eq!(
            request,
            ChatRequest {
                method: Method::GET,
                path: PathAndQuery::from_static("/v1/verification/session/aaabbbcccdddeee"),
                headers: HeaderMap::default(),
                body: None,
            }
        )
    }

    #[test]
    fn registration_update_session_request_as_chat_request() {
        let captcha_request: ChatRequest = RegistrationRequest {
            session_id: &SessionId::from_str("aaabbbcccdddeee").unwrap(),
            request: UpdateRegistrationSession {
                captcha: Some("captcha"),
                ..Default::default()
            },
        }
        .into();

        assert_eq!(
            captcha_request,
            ChatRequest {
                method: Method::PATCH,
                path: PathAndQuery::from_static("/v1/verification/session/aaabbbcccdddeee"),
                headers: HeaderMap::from_iter([CONTENT_TYPE_JSON]),
                body: Some(b"{\"captcha\":\"captcha\"}".as_slice().into())
            }
        );

        let captcha_request: ChatRequest = RegistrationRequest {
            session_id: &SessionId::from_str("aaabbbcccdddeee").unwrap(),
            request: UpdateRegistrationSession {
                push_token_type: Some(PushTokenType::Apn),
                ..Default::default()
            },
        }
        .into();

        assert_eq!(
            captcha_request,
            ChatRequest {
                method: Method::PATCH,
                path: PathAndQuery::from_static("/v1/verification/session/aaabbbcccdddeee"),
                headers: HeaderMap::from_iter([CONTENT_TYPE_JSON]),
                body: Some(b"{\"pushTokenType\":\"apn\"}".as_slice().into())
            }
        )
    }

    #[test]
    fn registration_request_verification_as_chat_request() {
        let captcha_request: ChatRequest = RegistrationRequest {
            session_id: &SessionId::from_str("aaabbbcccdddeee").unwrap(),
            request: RequestVerificationCode {
                transport: VerificationTransport::Sms,
                client: "client name",
            },
        }
        .into();

        assert_eq!(
            captcha_request,
            ChatRequest {
                method: Method::POST,
                path: PathAndQuery::from_static("/v1/verification/session/aaabbbcccdddeee/code"),
                headers: HeaderMap::from_iter([CONTENT_TYPE_JSON]),
                body: Some(
                    b"{\"transport\":\"sms\",\"client\":\"client name\"}"
                        .as_slice()
                        .into()
                )
            }
        );
    }

    #[test]
    fn registration_response_deserialize() {
        const RESPONSE_JSON: &str = r#"{
                "id": "fivesixseven",
                "allowedToRequestCode": true,
                "verified": true,
                "requestedInformation": ["pushChallenge", "captcha"]
            }"#;
        let response: RegistrationResponse = ChatResponse {
            status: StatusCode::OK,
            message: Some("OK".to_owned()),
            headers: HeaderMap::from_iter([CONTENT_TYPE_JSON]),
            body: Some(RESPONSE_JSON.as_bytes().into()),
        }
        .try_into_response()
        .unwrap();

        assert_eq!(
            response,
            RegistrationResponse {
                session_id: "fivesixseven".parse().unwrap(),
                session: RegistrationSession {
                    allowed_to_request_code: true,
                    verified: true,
                    next_sms: None,
                    next_call: None,
                    next_verification_attempt: None,
                    requested_information: HashSet::from([
                        RequestedInformation::Captcha,
                        RequestedInformation::PushChallenge
                    ]),
                }
            }
        );
    }

    static ACCOUNT_ATTRIBUTES: LazyLock<ProvidedAccountAttributes<'static>> =
        LazyLock::new(|| ProvidedAccountAttributes {
            recovery_password: b"recovery",
            registration_id: 123,
            pni_registration_id: 456,
            name: Some(b"device name proto"),
            registration_lock: Some("reg lock"),
            unidentified_access_key: Some(b"unidentified key"),
            unrestricted_unidentified_access: true,
            capabilities: HashSet::from(["can wear cape"]),
            discoverable_by_phone_number: true,
            each_registration_id_valid: Some(true),
        });

    static REGISTER_KEYS: LazyLock<(ForServiceIds<PublicKey>, ForServiceIds<SignedPreKeyRecord>)> =
        LazyLock::new(|| {
            // Use a seeded RNG for deterministic generation.
            let mut rng = rand_chacha::ChaChaRng::from_seed([1; 32]);

            let identity_keys = ForServiceIds::generate(|_| KeyPair::generate(&mut rng).public_key);

            let signed_pre_keys = ForServiceIds::generate(|_| {
                SignedPreKeyRecord::new(
                    1.into(),
                    libsignal_protocol::Timestamp::from_epoch_millis(42),
                    &KeyPair::generate(&mut rng),
                    b"signature",
                )
            });

            (identity_keys, signed_pre_keys)
        });

    /// "Golden" test that makes sure the auto-generated serialization code ends
    /// up producing the JSON we expect.
    #[test]
    fn register_account_request() {
        let (identity_keys, signed_pre_keys) = &*REGISTER_KEYS;

        // There's no good way to generate this deterministically. We just check
        // below that these keys appear in the correct spot in the generated
        // request.
        let kem_keypair =
            libsignal_protocol::kem::KeyPair::generate(libsignal_protocol::kem::KeyType::Kyber1024);
        let pq_last_resort_pre_keys = ForServiceIds::generate(|_| {
            KyberPreKeyRecord::new(
                1.into(),
                libsignal_protocol::Timestamp::from_epoch_millis(42),
                &kem_keypair,
                b"signature",
            )
        });

        let request = crate::chat::Request::register_account(
            Some(&"abc".parse().unwrap()),
            NewMessageNotification::Apn("appleId"),
            ACCOUNT_ATTRIBUTES.clone(),
            Some(SkipDeviceTransfer),
            ForServiceIds {
                aci: AccountKeys {
                    identity_key: &identity_keys.aci,
                    signed_pre_key: &signed_pre_keys.aci,
                    pq_last_resort_pre_key: &pq_last_resort_pre_keys.aci,
                },
                pni: AccountKeys {
                    identity_key: &identity_keys.pni,
                    signed_pre_key: &signed_pre_keys.pni,
                    pq_last_resort_pre_key: &pq_last_resort_pre_keys.pni,
                },
            },
            b"account password",
            "+18005550101",
        );

        let crate::chat::Request {
            method,
            body,
            headers,
            path,
        } = request;
        assert_eq!(path, "/v1/registration");
        assert_eq!(
            (method, headers),
            (
                Method::POST,
                HeaderMap::from_iter(
                    [
                        ("content-type", "application/json"),
                        (
                            "authorization",
                            "Basic KzE4MDA1NTUwMTAxOllXTmpiM1Z1ZENCd1lYTnpkMjl5WkE="
                        )
                    ]
                    .into_iter()
                    .map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))
                )
            )
        );
        let body = serde_json::from_slice::<'_, serde_json::Value>(&body.unwrap()).unwrap();
        print!(
            "actual body: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );

        pretty_assertions::assert_eq!(
            body,
            json!({
              "accountAttributes": {
                "capabilities": {
                  "can wear cape": true
                },
                "discoverableByPhoneNumber": true,
                "eachRegistrationIdValid": true,
                "fetchesMessages": false,
                "name": "ZGV2aWNlIG5hbWUgcHJvdG8=",
                "pniRegistrationId": 456,
                "recoveryPassword": "cmVjb3Zlcnk=",
                "registrationId": 123,
                "registrationLock": "reg lock",
                "unidentifiedAccessKey": [ 117, 110, 105, 100, 101, 110, 116, 105, 102, 105, 101, 100, 32, 107, 101, 121 ],
                "unrestrictedUnidentifiedAccess": true
              },
              "aciIdentityKey": "BdU7n+od1NVw2+OBgHZ8I2RWymYz8QPxqgY357YT0lJ0",
              "pniIdentityKey": "BQkeh2V1eV9fztQ/985a5lLbIeNFPGsexdO9I7HsQQZV",
              "aciSignedPreKey": {
                "keyId": 1,
                "publicKey": "BQ2BxG+rk+cP5r4EcBEzkU24jhR+Uh6YjC49E0BNgqEd",
                "signature": "c2lnbmF0dXJl"
              },
              "pniSignedPreKey": {
                "keyId": 1,
                "publicKey": "BbXFSRLIu8fIgPw0h1UFmwAUESqGkcNdWbYwolhBK8x6",
                "signature": "c2lnbmF0dXJl"
              },
              "pushToken": {
                "apnRegistrationId": "appleId"
              },
              "sessionId": "abc",
              "skipDeviceTransfer": true,
              // Not including the full serialized representation for these
              // since it's the same as the signed pre-keys so asserting
              // equality on them doesn't add value.
              "aciPqLastResortPreKey": SignedPrekeyBody::from(&pq_last_resort_pre_keys.aci),
              "pniPqLastResortPreKey": SignedPrekeyBody::from(&pq_last_resort_pre_keys.pni),
            })
        );
    }

    #[test]
    fn register_account_request_fetches_messages_no_push_tokens() {
        let pq_last_resort_pre_keys = ForServiceIds::generate(|_| {
            KyberPreKeyRecord::new(
                1.into(),
                libsignal_protocol::Timestamp::from_epoch_millis(42),
                &libsignal_protocol::kem::KeyPair::generate(
                    libsignal_protocol::kem::KeyType::Kyber1024,
                ),
                b"signature",
            )
        });

        let (identity_keys, signed_pre_keys) = &*REGISTER_KEYS;

        let request = crate::chat::Request::register_account(
            Some(&"abc".parse().unwrap()),
            NewMessageNotification::WillFetchMessages,
            ACCOUNT_ATTRIBUTES.clone(),
            Some(SkipDeviceTransfer),
            ForServiceIds {
                aci: AccountKeys {
                    identity_key: &identity_keys.aci,
                    signed_pre_key: &signed_pre_keys.aci,
                    pq_last_resort_pre_key: &pq_last_resort_pre_keys.aci,
                },
                pni: AccountKeys {
                    identity_key: &identity_keys.pni,
                    signed_pre_key: &signed_pre_keys.pni,
                    pq_last_resort_pre_key: &pq_last_resort_pre_keys.pni,
                },
            },
            b"account password",
            "+18005550101",
        );

        let body = serde_json::from_slice::<'_, serde_json::Value>(&request.body.unwrap()).unwrap();

        assert_eq!(
            body.get("accountAttributes")
                .and_then(|v| v.get("fetchesMessages")),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(body.get("pushToken"), None);
    }
}
