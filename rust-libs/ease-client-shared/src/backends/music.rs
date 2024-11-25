use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::define_id;

use super::{
    lyric::Lyrics,
    music_duration::MusicDuration,
    storage::{DataSourceKey, StorageEntryLoc},
};

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
    pub cover: Option<DataSourceKey>,
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
    pub cover: Option<DataSourceKey>,
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
            cover: self.cover.clone(),
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgUpdateMusicLyric {
    pub id: MusicId,
    pub lyric_loc: Option<StorageEntryLoc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimeToPauseInfo {
    pub enabled: bool,
    pub expired: Duration,
    pub left: Duration,
}
