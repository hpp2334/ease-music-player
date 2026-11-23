//! Lyric-parse dispatch — extension-based, plugin-provided.
//!
//! **No format knowledge lives in Rust.** Every parser (LRC included) is
//! a `contributions.lyricParsers` plugin contribution; the backend
//! registers a single `lyric:parse` host-RPC handler serving all of its
//! plugin's parsers (`parserId` rides the payload). This module resolves
//! candidate `(pluginId, parserId)` pairs for a file extension (scan
//! order, user selection from the Lyric Parser settings page applied),
//! ships the raw bytes to each candidate in turn over the RPC **binary
//! lane** (a `nil` reply / error / timeout falls through to the next),
//! and maps the winner's normalized result into the wire [`Lyrics`]
//! shape.
//!
//! Contract (see `plugins/com.ease.lyricformats/backend/`), the binary
//! lane's minimal framing — a small JSON header + the raw bytes:
//!
//! ```text
//! <JSON header>\n<content bytes>
//! ```
//!
//! The header carries the fields the parser needs (the plugin decodes
//! the whole payload lossily as UTF-8 and splits at the FIRST newline —
//! JSON strings never contain a literal `\n`):
//!
//! ```json
//! {"parserId": "lrc", "fileName": "song.lrc"}
//! ```
//!
//! The answer is raw UTF-8 bytes:
//!
//! ```json
//! {"lines": [{ "timeMs": 0, "durationMs": 1, "text": "…" }], "metadata"?}
//! ```
//!
//! or the nil answer ("not mine" → host tries the next parser).

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::objects::{LyricLine, Lyrics};

use super::plugin_manager::{LyricParserEntry, PluginManagerShared};

/// Hard cap on lyric bytes handed to a parser (raw bytes cross the binary
/// lane — keep lyric-sized anyway).
pub(crate) const MAX_LYRIC_BYTES: usize = 2 * 1024 * 1024;

/// Per-candidate RPC budget; a timeout falls through to the next parser.
const PARSE_TIMEOUT: Duration = Duration::from_secs(10);

/// Cap on result lines accepted from a plugin.
pub(crate) const MAX_LINES: usize = 65_536;

#[derive(Debug, thiserror::Error)]
pub(crate) enum LyricDispatchError {
    #[error("lyric file too large ({0} bytes > {})", MAX_LYRIC_BYTES)]
    TooLarge(usize),
    #[error("no lyric parser registered for '{0}'")]
    NoParser(String),
    #[error("all lyric parsers failed for '{0}'")]
    AllFailed(String),
}

/// Lowercase extension of a file name (after the last `.`), or `None` for
/// extensionless names / a trailing dot.
pub(crate) fn file_extension(file_name: &str) -> Option<String> {
    let base = file_name.rsplit('/').next().unwrap_or(file_name);
    let pos = base.rfind('.')?;
    let ext = base[pos + 1..].to_ascii_lowercase();
    if ext.is_empty() {
        None
    } else {
        Some(ext)
    }
}

/// Candidate parsers for `ext`, in dispatch order: scan order (plugins
/// sorted by id, contributions in manifest order), then the user's
/// selection (if it names a currently-registered enabled parser claiming
/// the extension) moved to the front — the remaining candidates keep
/// their order so `null`-fallthrough still works. Stale selections
/// (uninstalled/disabled plugin) are ignored. Pure function — unit
/// tested without a live RpcClient.
pub(crate) fn find_lyric_parsers(
    snapshot: &[LyricParserEntry],
    selection: &BTreeMap<String, String>,
    ext: &str,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for entry in snapshot {
        if !entry.enabled {
            continue;
        }
        for parser in &entry.parsers {
            if parser.extensions.iter().any(|e| e == ext) {
                out.push((entry.plugin_id.clone(), parser.id.clone()));
            }
        }
    }
    if let Some(key) = selection.get(ext) {
        let idx = out
            .iter()
            .position(|(pid, cid)| format!("{pid}:{cid}") == *key);
        if let Some(idx) = idx {
            let winner = out.remove(idx);
            out.insert(0, winner);
        }
    }
    out
}

/// Parse lyric bytes through the plugin chain. See the module docs for
/// the wire contract; result lines are sorted by time (stable), clamped
/// to `>= 0`, rounded to ms and capped.
pub(crate) async fn parse_lyric_content(
    cx: &crate::ctx::BackendContext,
    file_name: &str,
    bytes: &[u8],
) -> Result<Lyrics, LyricDispatchError> {
    if bytes.len() > MAX_LYRIC_BYTES {
        return Err(LyricDispatchError::TooLarge(bytes.len()));
    }
    let ext = file_extension(file_name)
        .ok_or_else(|| LyricDispatchError::NoParser(file_name.to_string()))?;
    let shared: &PluginManagerShared = cx.plugin_manager();
    let candidates = find_lyric_parsers(
        &shared.lyric_parser_snapshot(),
        &shared.lyric_parser_selection(),
        &ext,
    );
    if candidates.is_empty() {
        return Err(LyricDispatchError::NoParser(ext));
    }

    for (plugin_id, parser_id) in &candidates {
        let Some(rpc) = cx.service_rpc_for(plugin_id) else {
            // Backend not up (mid teardown / never loaded) — skip.
            continue;
        };
        let header = serde_json::to_vec(&json!({
            "parserId": parser_id,
            "fileName": file_name,
        }))
        .expect("the lyric header serializes");
        let mut payload = header;
        payload.push(b'\n');
        payload.extend_from_slice(bytes);
        let call = rpc.call_host_bytes("lyric:parse", payload);
        match tokio::time::timeout(PARSE_TIMEOUT, call).await {
            Ok(Ok(Some(reply))) => match map_plugin_result(&reply) {
                Some(lyrics) => return Ok(lyrics),
                None => {
                    tracing::warn!("lyric:parse '{plugin_id}:{parser_id}': unrecognized content");
                }
            },
            // The nil answer — "not mine", fall through silently (this is
            // the lane's designed multi-parser shape).
            Ok(Ok(None)) => {}
            Ok(Err(e)) => {
                tracing::warn!("lyric:parse '{plugin_id}:{parser_id}' failed: {e}");
            }
            Err(_) => {
                tracing::warn!("lyric:parse '{plugin_id}:{parser_id}' timed out");
            }
        }
    }
    Err(LyricDispatchError::AllFailed(ext))
}

/// Map a plugin's reply into [`Lyrics`]: permissive parse (unknown fields
/// ignored, `timeMs` accepts int/float), `None` when the reply is not a
/// usable result (empty/malformed lines — the host then falls through to
/// the next parser).
fn map_plugin_result(reply: &[u8]) -> Option<Lyrics> {
    let parsed: PluginLyricResult = serde_json::from_slice(reply).ok()?;
    if parsed.lines.is_empty() {
        return None;
    }
    let mut lines: Vec<LyricLine> = parsed
        .lines
        .into_iter()
        .filter_map(|l| {
            if !l.time_ms.is_finite() {
                return None;
            }
            let ms = l.time_ms.round().clamp(0.0, u64::MAX as f64) as u64;
            Some(LyricLine {
                duration: Duration::from_millis(ms),
                text: l.text,
            })
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    lines.sort_by(|a, b| a.duration.cmp(&b.duration));
    lines.truncate(MAX_LINES);
    let m = parsed.metadata.unwrap_or_default();
    Some(Lyrics {
        metdata: crate::objects::LrcMetadata {
            artist: m.artist,
            album: m.album,
            title: m.title,
            lyricist: m.lyricist,
            author: m.author,
            length: m.length,
            offset: m.offset,
        },
        lines,
        // The plugin contract is timed lines only — an unsynchronized
        // result never comes from a parser.
        synced: true,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginLyricResult {
    lines: Vec<PluginLyricLine>,
    metadata: Option<PluginLyricMetadata>,
}

#[derive(Deserialize)]
struct PluginLyricLine {
    #[serde(rename = "timeMs")]
    time_ms: f64,
    #[allow(dead_code)]
    #[serde(default)]
    duration_ms: Option<f64>,
    text: String,
}

/// Pass-through of tag values (mirrors [`crate::objects::LrcMetadata`]'s
/// all-strings shape — e.g. the LRC parser keeps `[offset:…]` /
/// `[length:…]` verbatim, exactly like the old Rust parser did).
#[derive(Deserialize, Default)]
struct PluginLyricMetadata {
    #[serde(default)]
    artist: String,
    #[serde(default)]
    album: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    lyricist: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    length: String,
    #[serde(default)]
    offset: String,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use super::{file_extension, find_lyric_parsers, map_plugin_result, MAX_LYRIC_BYTES};
    use crate::objects::LyricLine;
    use crate::services::plugin_manager::{LyricParserEntry, LyricParserRaw};

    fn entry(plugin_id: &str, enabled: bool, parsers: &[(&str, &[&str])]) -> LyricParserEntry {
        LyricParserEntry {
            plugin_id: plugin_id.to_string(),
            enabled,
            parsers: parsers
                .iter()
                .map(|(id, exts)| LyricParserRaw {
                    id: id.to_string(),
                    title: None,
                    desc: None,
                    icon: None,
                    icon_data: None,
                    extensions: exts.iter().map(|e| e.to_string()).collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn extension_extraction() {
        assert_eq!(file_extension("01 - Foo.SRT").as_deref(), Some("srt"));
        assert_eq!(file_extension("dir/a.b/q.rc").as_deref(), Some("rc"));
        assert_eq!(file_extension("noext").is_none(), true);
        assert_eq!(file_extension("trailing.").is_none(), true);
    }

    #[test]
    fn candidate_order_and_selection() {
        let snapshot = vec![
            entry("com.ease.a", true, &[("lrc", &["lrc"]), ("sub", &["srt", "vtt"])]),
            entry("com.ease.b", true, &[("srt2", &["srt"])]),
            entry("com.ease.c", false, &[("srt3", &["srt"])]), // disabled
        ];
        let no_sel = BTreeMap::new();
        // Scan order; disabled plugins skipped.
        let out = find_lyric_parsers(&snapshot, &no_sel, "srt");
        assert_eq!(
            out,
            vec![
                ("com.ease.a".to_string(), "sub".to_string()),
                ("com.ease.b".to_string(), "srt2".to_string()),
            ]
        );
        // Selection moves the winner to the front, keeps the rest.
        let sel = BTreeMap::from([("srt".to_string(), "com.ease.b:srt2".to_string())]);
        let out = find_lyric_parsers(&snapshot, &sel, "srt");
        assert_eq!(out[0], ("com.ease.b".to_string(), "srt2".to_string()));
        assert_eq!(out[1], ("com.ease.a".to_string(), "sub".to_string()));
        // Stale selection (unknown plugin) is ignored.
        let stale = BTreeMap::from([("srt".to_string(), "com.ease.gone:x".to_string())]);
        let out = find_lyric_parsers(&snapshot, &stale, "srt");
        assert_eq!(out[0].0, "com.ease.a");
        // No match.
        assert!(find_lyric_parsers(&snapshot, &no_sel, "krc").is_empty());
    }

    #[test]
    fn result_mapping_sorts_clamps_rounds() {
        let value = serde_json::json!({
            "lines": [
                {"timeMs": 61500, "text": "b"},
                {"timeMs": -3, "text": "a"},          // clamps to 0
                {"timeMs": 1200.6, "text": "c"},      // rounds to 1201
                {"timeMs": 123, "durationMs": 45, "unknown": 1, "text": "a2"},
            ],
            "metadata": {"artist": "X", "length": "03:45", "offset": "+500"},
        });
        let reply = serde_json::to_vec(&value).unwrap();
        let lyrics = map_plugin_result(&reply).unwrap();
        let times: Vec<u64> = lyrics
            .lines
            .iter()
            .map(|l| l.duration.as_millis() as u64)
            .collect();
        assert_eq!(times, vec![0, 123, 1201, 61500]);
        assert_eq!(lyrics.lines[3].text, "b");
        assert_eq!(lyrics.metdata.artist, "X");
        assert_eq!(lyrics.metdata.length, "03:45");
        assert_eq!(lyrics.metdata.offset, "+500");

        // Empty / malformed / non-object replies fall through.
        let empty = serde_json::to_vec(&serde_json::json!({"lines": []})).unwrap();
        assert!(map_plugin_result(&empty).is_none());
        let sparse = serde_json::to_vec(&serde_json::json!({"lines": [{"timeMs": 1}]})).unwrap();
        assert!(map_plugin_result(&sparse).is_none());
        assert!(map_plugin_result(b"not json").is_none());
        assert!(map_plugin_result(b"").is_none());
    }

    #[test]
    fn size_cap() {
        assert!(MAX_LYRIC_BYTES == 2 * 1024 * 1024);
    }
}
