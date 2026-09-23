use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LrcMetadata {
    pub artist: String,
    pub album: String,
    pub title: String,
    pub lyricist: String,
    pub author: String,
    pub length: String,
    pub offset: String,
}

#[serde_with::serde_as]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub duration: Duration,
    pub text: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub metdata: LrcMetadata,
    pub lines: Vec<LyricLine>,
    /// False = unsynchronized plain text (every line carries t=0): the
    /// client renders it without highlight / auto-scroll. Defaults to
    /// `true` on the wire so older payloads (and the plugin result
    /// mapping, which only ever produces synced lines) decode unchanged.
    #[serde(default = "default_true")]
    pub synced: bool,
}

fn default_true() -> bool {
    true
}
