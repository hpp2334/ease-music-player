//! Request shape for the bridge.

use serde::Deserialize;

/// Incoming request from the Kotlin side.
///
/// ```jsonc
/// // bare-arg method (e.g. playlist.get, music.get, player.seek):
/// { "method": "playlist.get", "args": 42, "handle": 1 }
///
/// // multi-field-arg method (e.g. playlist.update, player.loadMusic):
/// { "method": "playlist.update", "args": { "id": 42, "title": "..." }, "handle": 1 }
/// ```
///
/// - `method` selects the dispatcher branch.
/// - `args` is a raw JSON value; each branch deserializes it into the
///   concrete arg type expected by the underlying controller function.
///   For single-value args (PlaylistId, MusicId, String, numbers) the
///   value is sent bare; for multi-field args it's an object.
/// - `handle` is the opaque ID returned by `backend.create` /
///   `player.contextNew` / `player.new`. Omitted on the very first call.
#[derive(Debug, Deserialize)]
pub(crate) struct BridgeRequest {
    pub method: String,
    #[serde(default)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub handle: Option<u64>,
}
