use std::sync::Arc;

use misty_vm::views::MistyViewModelManagerBuilder;

use crate::{
    core_views::RootViewModelState,
    utils::{decode_component_or_origin, get_display_duration},
};

use super::{
    service::{AllPlaylistState, CreatePlaylistState, CurrentPlaylistState, EditPlaylistState},
    typ::*,
    Playlist,
};

fn playlist_list_view_model(state: &AllPlaylistState, root: &mut RootViewModelState) {
    let mut list: Vec<Arc<Playlist>> = { state.map.iter().map(|(_, item)| item.clone()).collect() };
    list.sort_by(|lhs, rhs| {
        rhs.created_time()
            .partial_cmp(&lhs.created_time())
            .unwrap_or(std::cmp::Ordering::Less)
    });

    let mut playlist_list: Vec<VPlaylistAbstractItem> = Default::default();

    for playlist in list.iter() {
        let duration = playlist.duration();
        playlist_list.push(VPlaylistAbstractItem {
            id: playlist.id().clone(),
            title: playlist.title().to_string(),
            count: playlist.musics().len() as i32,
            duration: get_display_duration(&duration),
            picture: if let Some(v) = playlist.self_picture().as_ref() {
                Some(*v.id())
            } else {
                playlist.first_picture_in_musics().as_ref().map(|v| *v.id())
            },
        });
    }

    let playlist_list_state = VPlaylistListState { playlist_list };

    root.playlist_list = Some(playlist_list_state);
}

fn current_playlist_view_model(
    (current_playlist, playlist_list): (&CurrentPlaylistState, &AllPlaylistState),
    root: &mut RootViewModelState,
) {
    let current_playlist_id = current_playlist.current_playlist_id.clone();
    let current_playlist = current_playlist_id
        .map(|id| playlist_list.map.get(&id).map(|v| v.clone()))
        .unwrap_or_default();

    if current_playlist.is_none() {
        root.current_playlist = None;
        return;
    }
    let current_playlist = current_playlist.unwrap();

    let items: Vec<VPlaylistMusicItem> = current_playlist
        .get_ordered_musics()
        .iter()
        .map(|music| {
            return VPlaylistMusicItem {
                id: music.music_id(),
                title: music.title().to_string(),
                duration: get_display_duration(&music.duration()),
            };
        })
        .collect();

    let current_playlist_state = VCurrentPlaylistState {
        id: Some(current_playlist.id()),
        items,
        title: current_playlist.title().to_string(),
        duration: get_display_duration(&current_playlist.duration()),
        picture: if let Some(v) = current_playlist.self_picture().as_ref() {
            Some(*v.id())
        } else {
            current_playlist
                .first_picture_in_musics()
                .as_ref()
                .map(|v| *v.id())
        },
        first_picture_in_musics: current_playlist
            .first_picture_in_musics()
            .clone()
            .map(|v| *v.id()),
    };

    root.current_playlist = Some(current_playlist_state);
}

fn edit_playlist_view_model(edit_playlist: &EditPlaylistState, root: &mut RootViewModelState) {
    let buf = edit_playlist.picture.as_ref().map(|buf| *buf.id());

    root.edit_playlist = Some(VEditPlaylistState {
        picture: buf,
        name: edit_playlist.playlist_name.clone(),
        prepared_signal: edit_playlist.prepared_signal,
    });
}

fn create_playlist_view_model(
    create_playlist: &CreatePlaylistState,
    root: &mut RootViewModelState,
) {
    let mode = create_playlist.mode;
    let buf = create_playlist.picture.as_ref().map(|buf| *buf.id());
    let music_count = match &create_playlist.entries {
        Some(v) => v.entries.len(),
        None => 0,
    };

    root.create_playlist = Some(VCreatePlaylistState {
        mode,
        music_count: music_count as u32,
        picture: buf,
        recommend_playlist_names: create_playlist
            .recommend_playlist_names
            .clone()
            .into_iter()
            .map(decode_component_or_origin)
            .collect(),
        name: decode_component_or_origin(create_playlist.playlist_name.clone()),
        prepared_signal: create_playlist.prepared_signal,
        full_imported: create_playlist.full_imported,
    });
}

pub fn register_playlist_viewmodels(
    builder: MistyViewModelManagerBuilder<RootViewModelState>,
) -> MistyViewModelManagerBuilder<RootViewModelState> {
    builder
        .register(playlist_list_view_model)
        .register(current_playlist_view_model)
        .register(edit_playlist_view_model)
        .register(create_playlist_view_model)
}
