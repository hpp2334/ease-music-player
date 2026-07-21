use std::time::Duration;

use ease_client_schema::{DataSourceKey, MusicId, StorageEntryLoc};
use ease_order_key::OrderKey;

use super::lyric::Lyrics;

#[derive(Debug, Clone, uniffi::Record)]
pub struct MusicMeta {
    pub id: MusicId,
    pub title: String,
    pub duration: Option<Duration>,
    pub order: Vec<u32>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct MusicAbstract {
    pub meta: MusicMeta,
    pub cover: Option<DataSourceKey>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LyricLoadState {
    Loading,
    #[default]
    Missing,
    Failed,
    Loaded,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct MusicLyric {
    pub loc: StorageEntryLoc,
    pub data: Lyrics,
    pub loaded_state: LyricLoadState,
}

#[derive(Debug, Clone, uniffi::Record)]
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
    pub fn duration(&self) -> Option<Duration> {
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
    pub fn duration(&self) -> Option<Duration> {
        self.meta.duration
    }
}

// ============================================================================
// Player-facing records (mirrors of cantode types, UniFFI-friendly).
// ============================================================================

/// PCM format of a decoded audio source.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AudioFormatRecord {
    /// Channel count (1 = mono, 2 = stereo, ...).
    pub channels: u16,
    /// Sample rate in Hz (e.g. 44100).
    pub sample_rate: u32,
}

/// A single free-form metadata tag.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TagRecord {
    pub key: String,
    pub value: String,
}

/// Probed metadata for an audio source, surfaced across UniFFI.
///
/// Durations are in milliseconds (UniFFI's `Duration` mapping adds friction
/// we don't need for a UI-consumable record). Cover-art bytes are NOT
/// included here — fetch them separately via `ct_get_asset(DataSourceKey::Cover)`
/// when needed. The `has_cover` flag lets the UI decide whether to bother.
#[derive(Debug, Clone, uniffi::Record)]
pub struct MetadataRecord {
    pub format: AudioFormatRecord,
    /// Total duration in milliseconds, if known.
    pub duration_ms: Option<u64>,
    pub tags: Vec<TagRecord>,
    /// Whether the source carried embedded cover art. (The bytes are
    /// available via `ct_get_asset(DataSourceKey::Cover)`; for freshly-
    /// loaded musics without a DB cover, the backend writes the probed
    /// bytes back asynchronously — see `ct_player_load_music`.)
    pub has_cover: bool,
}

impl MetadataRecord {
    /// Build a [`MetadataRecord`] from a [`cantode::Metadata`].
    ///
    /// Internal: only the player controller calls this.
    pub(crate) fn from_cantode(m: &cantode::Metadata) -> Self {
        Self {
            format: AudioFormatRecord {
                channels: m.format.channels,
                sample_rate: m.format.sample_rate,
            },
            duration_ms: m.duration.map(|d| d.as_millis() as u64),
            tags: m
                .tags
                .iter()
                .map(|t| TagRecord {
                    key: t.key.clone(),
                    value: t.value.clone(),
                })
                .collect(),
            has_cover: m.cover_art.is_some(),
        }
    }
}

/// Mirror of [`cantode::PlayerState`] for the FFI surface.
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum PlayerStateRecord {
    Idle,
    Loading,
    Paused,
    Playing,
    Buffering,
    Ended,
    Error,
}

impl PlayerStateRecord {
    /// Map a [`cantode::PlayerState`] to its record form.
    pub(crate) fn from_cantode(s: cantode::PlayerState) -> Self {
        match s {
            cantode::PlayerState::Idle => Self::Idle,
            cantode::PlayerState::Loading => Self::Loading,
            cantode::PlayerState::Paused => Self::Paused,
            cantode::PlayerState::Playing => Self::Playing,
            cantode::PlayerState::Buffering => Self::Buffering,
            cantode::PlayerState::Ended => Self::Ended,
            cantode::PlayerState::Error => Self::Error,
        }
    }
}
