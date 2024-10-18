use ease_client_shared::uis::{
    playlist::{
        VCreatePlaylistState, VCurrentPlaylistState, VEditPlaylistState, VPlaylistAbstractItem,
        VPlaylistListState, VPlaylistMusicItem,
    },
    view::RootViewModelState,
};
use misty_vm::views::MistyViewModelManagerBuilder;

use crate::utils::{decode_component_or_origin, get_display_duration};

use super::service::{
    AllPlaylistState, CreatePlaylistState, CurrentPlaylistState, EditPlaylistState,
};

fn playlist_list_view_model(state: &AllPlaylistState, root: &mut RootViewModelState) {
    let mut list: Vec<_> = { state.list.iter().map(|item| item.clone()).collect() };
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

fn current_playlist_view_model(
    (current_playlist): (&CurrentPlaylistState),
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
                id: music.id,
                title: music.title.to_string(),
                duration: get_display_duration(&music.duration),
            };
        })
        .collect();

    let current_playlist_state = VCurrentPlaylistState {
        id: Some(playlist.id()),
        items,
        title: playlist.title().to_string(),
        duration: get_display_duration(&playlist.duration()),
        cover_url: playlist.cover_url().to_string(),
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
