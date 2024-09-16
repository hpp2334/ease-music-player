use serde::Serialize;

use crate::backends::{music::MusicId, playlist::PlaylistId};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum CreatePlaylistMode {
    #[default]
    Full,
    Empty,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct VPlaylistAbstractItem {
    pub id: PlaylistId,
    pub title: String,
    pub count: i32,
    pub duration: String,
    pub picture: Option<u64>,
}

#[derive(Debug, Clone, Serialize, uniffi::Record)]
pub struct VPlaylistMusicItem {
    pub id: MusicId,
    pub title: String,
    pub duration: String,
}

#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct VPlaylistListState {
    pub playlist_list: Vec<VPlaylistAbstractItem>,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VCurrentPlaylistState {
    pub id: Option<PlaylistId>,
    pub items: Vec<VPlaylistMusicItem>,
    pub title: String,
    pub duration: String,
    pub picture: Option<u64>,
    pub first_picture_in_musics: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VEditPlaylistState {
    pub picture: Option<u64>,
    pub name: String,
    pub prepared_signal: u16,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VCreatePlaylistState {
    pub mode: CreatePlaylistMode,
    pub name: String,
    pub picture: Option<u64>,
    pub music_count: u32,
    pub recommend_playlist_names: Vec<String>,
    pub prepared_signal: u16,
    pub full_imported: bool,
}
