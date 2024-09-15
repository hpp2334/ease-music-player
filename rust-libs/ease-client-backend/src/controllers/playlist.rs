use std::{collections::HashMap, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    core::{result::ChannelResult, schema::IMessage},
    ctx::Context,
    define_message,
    models::{
        music::MusicId,
        playlist::{PlaylistId, PlaylistModel},
    },
    repositories::{
        core::get_conn,
        music::{db_add_music, db_load_music_metas_by_playlist_id, ArgDBAddMusic},
        playlist::{
            db_batch_add_music_to_playlist, db_load_first_music_covers, db_load_playlists,
            db_upsert_playlist, ArgDBUpsertPlaylist, FirstMusicCovers,
        },
    },
};

use super::{
    code::Code,
    music::{build_music_meta, MusicMeta},
    storage::StorageEntry,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaylistMeta {
    pub id: PlaylistId,
    pub title: String,
    pub cover_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Playlist {
    pub meta: PlaylistMeta,
    pub musics: Vec<MusicMeta>,
}

fn build_playlist_meta(
    cx: &Context,
    model: PlaylistModel,
    first_covers: &FirstMusicCovers,
) -> PlaylistMeta {
    let cover = if let Some(picture) = model.picture {
        Some(picture)
    } else {
        first_covers
            .get(&model.id)
            .map(|c| c.clone())
            .unwrap_or_default()
    };
    let cover_url = if let Some(cover) = cover {
        cx.server.add_image(cover)
    } else {
        Default::default()
    };

    PlaylistMeta {
        id: model.id,
        title: model.title,
        cover_url,
    }
}

define_message!(
    GetAllPlaylistMetasMsg,
    Code::GetAllPlaylistMetas,
    (),
    Vec<PlaylistMeta>
);
pub(crate) async fn cr_get_all_playlist_metas(
    cx: Context,
    _arg: (),
) -> anyhow::Result<Vec<PlaylistMeta>> {
    let conn = get_conn(&cx)?;
    let models = db_load_playlists(conn.get_ref())?;
    let first_covers = db_load_first_music_covers(conn.get_ref())?;

    let ret: Vec<PlaylistMeta> = models
        .into_iter()
        .map(|model| build_playlist_meta(&cx, model, &first_covers))
        .collect();
    Ok(ret)
}

define_message!(
    GetPlaylistMsg,
    Code::GetPlaylist,
    PlaylistId,
    Option<Playlist>
);
pub(crate) async fn cr_get_playlist(
    cx: Context,
    arg: PlaylistId,
) -> anyhow::Result<Option<Playlist>> {
    let conn = get_conn(&cx)?;
    let models = db_load_playlists(conn.get_ref())?;
    let first_covers = db_load_first_music_covers(conn.get_ref())?;

    let model = models.into_iter().find(|m| m.id == arg);
    if model.is_none() {
        return Ok(None);
    }
    let model = model.unwrap();
    let meta = build_playlist_meta(&cx, model, &first_covers);

    let musics = db_load_music_metas_by_playlist_id(conn.get_ref(), arg)?;
    let musics = musics.into_iter().map(|v| build_music_meta(v)).collect();

    Ok(Some(Playlist { meta, musics }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgUpdatePlaylist {
    id: PlaylistId,
    title: String,
    picture: Option<StorageEntry>,
    current_time_ms: i64,
}
define_message!(
    UpdatePlaylistMsg,
    Code::UpdatePlaylist,
    ArgUpdatePlaylist,
    ()
);
pub(crate) async fn cr_update_playlist(cx: Context, arg: ArgUpdatePlaylist) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    let current_time_ms = arg.current_time_ms;
    let arg: ArgDBUpsertPlaylist = ArgDBUpsertPlaylist {
        id: Some(arg.id),
        title: arg.title,
        picture: arg.picture.map(|v| (v.storage_id, v.path)),
    };
    db_upsert_playlist(conn.get_ref(), arg, current_time_ms);
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArgAddMusicsToPlaylist {
    id: PlaylistId,
    entries: Vec<(StorageEntry, String)>,
}
define_message!(
    AddMusicsToPlaylist,
    Code::AddMusicsToPlaylist,
    ArgAddMusicsToPlaylist,
    ()
);
pub(crate) async fn cr_add_musics_to_playlist(
    cx: Context,
    arg: ArgAddMusicsToPlaylist,
) -> anyhow::Result<()> {
    let conn = get_conn(&cx)?;
    let playlist_id = arg.id;
    let mut musics: Vec<(MusicId, PlaylistId)> = Default::default();

    for entry in arg.entries {
        let music_id = db_add_music(
            conn.get_ref(),
            ArgDBAddMusic {
                storage_id: entry.0.storage_id,
                path: entry.0.path,
                title: entry.1,
            },
        )?;
        musics.push((music_id, playlist_id));
    }
    db_batch_add_music_to_playlist(conn.get_ref(), musics)?;

    Ok(())
}
