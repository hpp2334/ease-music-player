//! Unified JSON + buffer bridge.
//!
//! Replaces the UniFFI-exposed surface with a single dispatcher entrypoint.
//! The Kotlin side invokes it through one JNI symbol
//! ([`crate::bridge::jni::Java_com_kutedev_easemusicplayer_singleton_EaseBridge_call`])
//! that takes a JSON payload string and an array of byte arrays. The
//! dispatcher parses the request, calls the existing `ct_*` / `cts_*`
//! controller functions, and serializes the response back into the
//! envelope:
//!
//! ```jsonc
//! // success
//! { "success": true, "payload": <T> }
//! // error (BError serialized via #[serde(tag = "errorCode", content = "errorDetail")])
//! { "success": false, "errorCode": "MusicNotFound", "errorDetail": { "value": 42 } }
//! ```
//!
//! Binary payloads (cover art, future bulk reads) cross via the `buffer`
//! side-channel — an `Array<ByteArray>` on the Kotlin side. The payload
//! references elements by `bytesIndex: N`.

pub(crate) mod dispatch;
pub(crate) mod handle_table;
pub(crate) mod jni;
pub(crate) mod request;
