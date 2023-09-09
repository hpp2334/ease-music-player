use std::time::Duration;

use bytes::Bytes;
use lofty::AudioFile;

mod lyrics;

pub use lyrics::{parse_lrc, LrcMetadata, LrcParseError, Lyrics};

pub enum MTag {
    ID3 {
        tag: MResult<id3::Tag>,
        lofty_file: Option<lofty::TaggedFile>,
    },
}

#[derive(Debug, Clone)]
pub struct MPic {
    pub mime_type: String,
    pub buf: Bytes,
}

#[derive(thiserror::Error, Debug)]
pub enum MError {
    #[error("Read Tag Error")]
    ReadTag(String),
}

type MResult<T> = Result<T, MError>;

impl MTag {
    pub fn read_from(bytes: Bytes) -> MResult<Self> {
        let tag = id3::Tag::read_from(std::io::Cursor::new(bytes.clone().to_vec()))
            .map_err(|e| MError::ReadTag(e.description));

        let buf_cursor = std::io::Cursor::new(bytes.to_vec());
        let file = lofty::Probe::new(std::io::BufReader::new(buf_cursor))
            .guess_file_type()
            .ok()
            .map(|f| f.read())
            .map(|f| f.ok())
            .unwrap_or_default();

        Ok(MTag::ID3 {
            tag,
            lofty_file: file,
        })
    }

    pub fn pic(&self) -> Option<MPic> {
        match &self {
            MTag::ID3 { tag, lofty_file: _ } => {
                if tag.is_err() {
                    return None;
                }
                let pic = tag.as_ref().unwrap().pictures().next();
                let pic = pic.map(|pic| {
                    let buf = Bytes::copy_from_slice(&pic.data);
                    let mime_type = pic.mime_type.clone();
                    MPic { mime_type, buf }
                });
                pic
            }
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        match &self {
            MTag::ID3 { tag: _, lofty_file } => {
                if lofty_file.is_none() {
                    return None;
                }
                let music_properties = lofty_file.as_ref().unwrap().properties();
                Some(music_properties.duration())
            }
        }
    }
}
