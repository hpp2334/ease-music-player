use std::time::Duration;

#[derive(Debug, PartialEq, Eq, Default, Clone, uniffi::Record)]
pub struct LrcMetadata {
    pub artist: String,
    pub album: String,
    pub title: String,
    pub lyricist: String,
    pub author: String,
    pub length: String,
    pub offset: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LyricLine {
    pub duration: Duration,
    pub text: String,
}

#[derive(Debug, Default, Clone, uniffi::Record)]
pub struct Lyrics {
    pub metdata: LrcMetadata,
    pub lines: Vec<LyricLine>,
}
