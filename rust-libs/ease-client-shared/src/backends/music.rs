use misty_serve::define_message;
use serde::{Deserialize, Serialize};

use crate::{backends::code::Code, define_id};

use super::{lyric::Lyrics, music_duration::MusicDuration, storage::StorageEntryLoc};

define_id!(MusicId);

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicMeta {
    pub id: MusicId,
    pub title: String,
    pub duration: Option<MusicDuration>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicLyric {
    pub loc: StorageEntryLoc,
    pub data: Lyrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Music {
    pub meta: MusicMeta,
    pub loc: StorageEntryLoc,
    pub picture_loc: Option<StorageEntryLoc>,
    pub lyric: Option<MusicLyric>,
}

define_message!(GetMusicMsg, Code::GetMusic, MusicId, ());

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgUpdateMusicDuration {
    pub id: MusicId,
    pub duration: MusicDuration,
}
define_message!(
    UpdateMusicDurationMsg,
    Code::UpdateMusicDuration,
    ArgUpdateMusicDuration,
    ()
);

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgUpdateMusicCover {
    pub id: MusicId,
    pub cover_loc: Option<StorageEntryLoc>,
}
define_message!(
    UpdateMusicCoverMsg,
    Code::UpdateMusicCover,
    ArgUpdateMusicCover,
    ()
);

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgUpdateMusicLyric {
    pub id: MusicId,
    pub lyric_loc: Option<StorageEntryLoc>,
}
define_message!(
    UpdateMusicLyricMsg,
    Code::UpdateMusicLyric,
    ArgUpdateMusicLyric,
    ()
);
