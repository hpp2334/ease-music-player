


use serde::Serialize;

use crate::{define_id, modules::music::MusicId};

define_id!(PlaylistId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CreatePlaylistMode {
    Full,
    Empty,
}

impl Default for CreatePlaylistMode {
    fn default() -> Self {
        Self::Full
    }
}

#[derive(Debug, Clone)]
pub struct VPlaylistAbstractItem {
    pub id: PlaylistId,
    pub title: String,
    pub count: i32,
    pub duration: String,
    pub picture: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VPlaylistMusicItem {
    pub id: MusicId,
    pub title: String,
    pub duration: String,
}

#[derive(Debug, Clone, Default)]
pub struct VPlaylistListState {
    pub playlist_list: Vec<VPlaylistAbstractItem>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VCurrentPlaylistState {
    pub id: Option<PlaylistId>,
    pub items: Vec<VPlaylistMusicItem>,
    pub title: String,
    pub duration: String,
    pub picture: Option<u64>,
    pub first_picture_in_musics: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VEditPlaylistState {
    pub picture: Option<u64>,
    pub name: String,
    pub prepared_signal: u16,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VCreatePlaylistState {
    pub mode: CreatePlaylistMode,
    pub name: String,
    pub picture: Option<u64>,
    pub music_count: u32,
    pub recommend_playlist_names: Vec<String>,
    pub prepared_signal: u16,
    pub full_imported: bool,
}
