use ease_client_shared::backends::{
    music::MusicId,
    playlist::{CreatePlaylistMode, PlaylistId},
};
use serde::Serialize;

use crate::{
    utils::common::{decode_component_or_origin, get_display_duration},
    view_models::{
        connector::state::ConnectorState,
        playlist::state::{
            AllPlaylistState, CreatePlaylistState, CurrentPlaylistState, EditPlaylistState,
        },
    },
};

use super::models::RootViewModelState;

#[derive(Debug, Clone, uniffi::Record)]
pub struct VPlaylistAbstractItem {
    pub id: PlaylistId,
    pub title: String,
    pub count: i32,
    pub duration: String,
    pub cover_url: String,
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
    pub cover_url: String,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VEditPlaylistState {
    pub picture: String,
    pub name: String,
    pub modal_open: bool,
}

#[derive(Debug, Clone, Default, Serialize, uniffi::Record)]
pub struct VCreatePlaylistState {
    pub mode: CreatePlaylistMode,
    pub name: String,
    pub picture: String,
    pub music_count: u32,
    pub recommend_playlist_names: Vec<String>,
    pub full_imported: bool,
    pub modal_open: bool,
    pub can_submit: bool,
}

pub(crate) fn playlist_list_vs(
    (state, connector_state): (&AllPlaylistState, &ConnectorState),
    root: &mut RootViewModelState,
) {
    let mut list: Vec<_> = { state.playlists.iter().map(|item| item.clone()).collect() };
    list.sort_by(|lhs, rhs| {
        rhs.created_time()
            .partial_cmp(lhs.created_time())
            .unwrap_or(std::cmp::Ordering::Less)
    });

    let mut playlist_list: Vec<VPlaylistAbstractItem> = Default::default();

    for playlist in list.iter() {
        let duration = playlist.duration;
        playlist_list.push(VPlaylistAbstractItem {
            id: playlist.id(),
            title: playlist.title().to_string(),
            count: playlist.music_count as i32,
            duration: get_display_duration(&duration),
            cover_url: playlist.cover_url().to_string(),
        });
    }

    let playlist_list_state = VPlaylistListState { playlist_list };

    root.playlist_list = Some(playlist_list_state);
}

pub(crate) fn current_playlist_vs(
    (current_playlist, connector_state): (&CurrentPlaylistState, &ConnectorState),
    root: &mut RootViewModelState,
) {
    let playlist = current_playlist.playlist.clone();

    if playlist.is_none() {
        root.current_playlist = None;
        return;
    }
    let playlist = playlist.unwrap();

    let items: Vec<VPlaylistMusicItem> = playlist
        .musics
        .iter()
        .map(|music| {
            return VPlaylistMusicItem {
                id: music.id(),
                title: music.title().to_string(),
                duration: get_display_duration(&music.duration()),
            };
        })
        .collect();

    let current_playlist_state = VCurrentPlaylistState {
        id: Some(playlist.id()),
        items,
        title: playlist.title().to_string(),
        duration: get_display_duration(&playlist.duration()),
        cover_url: connector_state.serve_asset_url_opt(playlist.cover().clone()),
    };

    root.current_playlist = Some(current_playlist_state);
}

pub(crate) fn edit_playlist_vs(
    (edit_playlist, connector_state): (&EditPlaylistState, &ConnectorState),
    root: &mut RootViewModelState,
) {
    root.edit_playlist = Some(VEditPlaylistState {
        picture: connector_state.serve_asset_url_opt(edit_playlist.cover.clone()),
        name: edit_playlist.playlist_name.clone(),
        modal_open: edit_playlist.modal_open,
    });
}

pub(crate) fn create_playlist_vs(
    (create_playlist, connector_state): (&CreatePlaylistState, &ConnectorState),
    root: &mut RootViewModelState,
) {
    let mode = create_playlist.mode;
    let cover = connector_state.serve_asset_url_opt(create_playlist.cover.clone());
    let music_count = create_playlist.entries.len();

    root.create_playlist = Some(VCreatePlaylistState {
        mode,
        music_count: music_count as u32,
        picture: cover,
        recommend_playlist_names: create_playlist.recommend_playlist_names.clone(),
        name: decode_component_or_origin(create_playlist.playlist_name.clone()),
        full_imported: create_playlist.mode == CreatePlaylistMode::Full
            && (!create_playlist.entries.is_empty() || create_playlist.cover.is_some()),
        modal_open: create_playlist.modal_open,
        can_submit: !create_playlist.playlist_name.is_empty(),
    });
}
