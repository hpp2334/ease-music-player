use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LrcMetadata {
    pub artist: String,
    pub album: String,
    pub title: String,
    pub lyricist: String,
    pub author: String,
    pub length: String,
    pub offset: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Lyrics {
    pub metdata: LrcMetadata,
    pub lines: Vec<(Duration, String)>,
}
