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
}
