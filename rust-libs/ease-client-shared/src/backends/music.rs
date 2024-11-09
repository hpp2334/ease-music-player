use serde::{Deserialize, Serialize};

use crate::{backends::code::Code, define_id, define_message};

use super::{lyric::Lyrics, music_duration::MusicDuration, storage::StorageEntryLoc};

define_id!(MusicId);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MusicMeta {
    pub id: MusicId,
    pub title: String,
    pub duration: Option<MusicDuration>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MusicAbstract {
    pub meta: MusicMeta,
    pub cover_url: String,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum LyricLoadState {
    Loading,
    #[default]
    Missing,
    Failed,
    Loaded,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MusicLyric {
    pub loc: StorageEntryLoc,
    pub data: Lyrics,
    pub loaded_state: LyricLoadState,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Music {
    pub meta: MusicMeta,
    pub loc: StorageEntryLoc,
    pub url: String,
    pub cover_loc: Option<StorageEntryLoc>,
    pub cover_url: String,
    pub lyric: Option<MusicLyric>,
}

impl Music {
    pub fn id(&self) -> MusicId {
        self.meta.id
    }
    pub fn duration(&self) -> Option<MusicDuration> {
        self.meta.duration
    }
    pub fn title(&self) -> &str {
        &self.meta.title
    }
    pub fn music_abstract(&self) -> MusicAbstract {
        MusicAbstract {
            meta: self.meta.clone(),
            cover_url: self.cover_url.clone(),
        }
    }
}

impl MusicAbstract {
    pub fn id(&self) -> MusicId {
        self.meta.id
    }
    pub fn title(&self) -> &str {
        &self.meta.title
    }
    pub fn duration(&self) -> Option<MusicDuration> {
        self.meta.duration
    }
}

define_message!(GetMusicMsg, Code::GetMusic, MusicId, Option<Music>);

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
