use std::collections::HashMap;

use ease_database::{params, DbConnectionRef};
use misty_vm::{
    client::MistyClientHandle,
    resources::{MistyResourceHandle, MistyResourceManager},
};

use crate::modules::{app::service::get_db_conn_v2, music::repository::MusicDuration};
use crate::modules::{
    error::EaseResult, playlist::repository::PlaylistModel, MusicId, PlaylistId, StorageId,
};

use super::model::*;
use super::{Playlist, PlaylistMusic};

pub struct ArgDBUpsertPlaylist {
    pub id: Option<PlaylistId>,
    pub title: String,
    pub picture: Option<Vec<u8>>,
}
pub fn db_upsert_playlist(
    app: MistyClientHandle,
    arg: ArgDBUpsertPlaylist,
    current_time_ms: i64,
) -> EaseResult<PlaylistId> {
    let conn = get_db_conn_v2(app)?;
    if arg.id.is_some() {
        conn.execute(
            "UPDATE playlist SET title = ?1, picture = ?2 WHERE id = ?3",
            params![arg.title, arg.picture, arg.id.unwrap()],
        )?;
        Ok(arg.id.unwrap())
    } else {
        let id = conn.query::<PlaylistId>(
            "INSERT INTO playlist (title, picture, created_time) VALUES (?1, ?2, ?3) RETURNING id",
            params![arg.title, arg.picture, current_time_ms],
        )?.pop().unwrap();
        Ok(id)
    }
}

pub(in crate::modules::playlist) fn db_remove_playlist(
    app: MistyClientHandle,
    id: PlaylistId,
) -> EaseResult<()> {
    let conn = get_db_conn_v2(app)?;
    conn.execute("DELETE FROM playlist WHERE id = ?1", params![id])?;
    Ok(())
}

pub(in crate::modules::playlist) fn db_remove_all_musics_in_playlist(
    app: MistyClientHandle,
    id: PlaylistId,
) -> EaseResult<()> {
    let conn = get_db_conn_v2(app)?;
    conn.execute(
        "DELETE FROM playlist_music WHERE playlist_id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn db_remove_music_from_playlist(
    app: MistyClientHandle,
    playlist_id: PlaylistId,
    music_id: MusicId,
) -> EaseResult<()> {
    let conn = get_db_conn_v2(app)?;
    conn.execute(
        "DELETE FROM playlist_music WHERE playlist_id = ?1 AND music_id = ?2",
        params![playlist_id, music_id],
    )?;
    Ok(())
}

pub fn db_batch_add_music_to_playlist(
    app: MistyClientHandle,
    args: Vec<(MusicId, PlaylistId)>,
) -> EaseResult<()> {
    let conn = get_db_conn_v2(app)?;

    for (music_id, playlist_id) in args {
        conn.execute(
            "INSERT OR IGNORE INTO playlist_music (playlist_id, music_id) VALUES (?1, ?2)",
            params![playlist_id, music_id],
        )?;
    }
    Ok(())
}

fn db_load_playlists_impl(app: MistyClientHandle) -> EaseResult<HashMap<PlaylistId, Playlist>> {
    let conn = get_db_conn_v2(app)?;
    let playlist_models = conn.query::<PlaylistModel>(
        r#"
        SELECT id, title, picture, created_time FROM playlist;
    "#,
        [],
    )?;

    let mut playlists: HashMap<PlaylistId, Playlist> = playlist_models
        .into_iter()
        .map(|model| Playlist {
            model,
            musics: Default::default(),
            ordered_music_ids: Default::default(),
            self_picture: Default::default(),
            picture_owning_music: Default::default(),
            first_picture_in_musics: Default::default(),
        })
        .map(|mut p| {
            if let Some(picture) = p.model.picture.take() {
                p.self_picture = Some(app.resource_manager().insert(picture));
            }
            (p.model.id, p)
        })
        .collect();

    for (music_id, playlist_id, first_picture) in
        db_load_first_music_cover_in_playlist_impl(conn.get_ref(), app.resource_manager())?
            .into_iter()
    {
        if let Some(p) = playlists.get_mut(&playlist_id) {
            p.first_picture_in_musics = Some(first_picture);
            p.picture_owning_music = Some(music_id);
        }
    }

    for (id, playlist_music) in db_load_playlist_musics(conn.get_ref())?.into_iter() {
        if let Some(p) = playlists.get_mut(&playlist_music.playlist_id()) {
            p.musics.insert(id, playlist_music);
        }
    }

    for (_, p) in playlists.iter_mut() {
        recalc_playlist_ordered_music_ids(p);
    }

    return Ok(playlists);
}

pub(in crate::modules::playlist) fn db_load_playlists_full(
    app: MistyClientHandle,
) -> EaseResult<HashMap<PlaylistId, Playlist>> {
    db_load_playlists_impl(app)
}

pub(in crate::modules::playlist) fn db_load_single_playlist_full(
    app: MistyClientHandle,
    id: PlaylistId,
) -> EaseResult<Option<Playlist>> {
    let mut map = db_load_playlists_impl(app)?;
    Ok(map.remove(&id))
}

fn db_load_first_music_cover_in_playlist_impl(
    conn: DbConnectionRef,
    resource_manager: &MistyResourceManager,
) -> EaseResult<Vec<(MusicId, PlaylistId, MistyResourceHandle)>> {
    let list = conn.query::<(MusicId, PlaylistId, Option<Vec<u8>>)>(
        r#"
    SELECT music_id, playlist_id, picture
    FROM playlist_music
    JOIN music ON music.id = playlist_music.music_id
    WHERE music.picture NOT NULL
    GROUP BY playlist_id;
"#,
        [],
    )?;

    let list = list
        .into_iter()
        .map(|(music_id, playlist_id, picture)| {
            let picture = resource_manager.insert(picture.unwrap());

            (music_id, playlist_id, picture)
        })
        .collect();

    Ok(list)
}

pub(in crate::modules::playlist) fn db_load_first_music_cover_in_playlist(
    app: MistyClientHandle,
) -> EaseResult<Vec<(MusicId, PlaylistId, MistyResourceHandle)>> {
    let conn = get_db_conn_v2(app)?;
    db_load_first_music_cover_in_playlist_impl(conn.get_ref(), app.resource_manager())
}

pub(in crate::modules::playlist) fn db_get_playlist_music_tuples(
    client: MistyClientHandle,
    storage_id: StorageId,
) -> EaseResult<HashMap<PlaylistId, Vec<MusicId>>> {
    let conn = get_db_conn_v2(client)?;

    let tuples = conn.query::<(PlaylistId, MusicId)>(
        r#"
    SELECT playlist_id, music_id FROM playlist_music
    WHERE music_id IN (SELECT id FROM music WHERE storage_id = ?1)
    "#,
        params![storage_id],
    )?;

    let mut map: HashMap<PlaylistId, Vec<MusicId>> = Default::default();
    for (playlist_id, music_id) in tuples.into_iter() {
        map.entry(playlist_id).or_default().push(music_id);
    }

    Ok(map)
}

pub(in crate::modules::playlist) fn db_remove_musics_in_playlists_by_storage(
    client: MistyClientHandle,
    storage_id: StorageId,
) -> EaseResult<()> {
    let conn = get_db_conn_v2(client)?;

    conn.execute(
        r#"DELETE FROM playlist_music
    WHERE id IN (SELECT id FROM music WHERE storage_id = ?1)"#,
        params![storage_id],
    )?;
    Ok(())
}

fn db_load_playlist_musics(conn: DbConnectionRef) -> EaseResult<HashMap<MusicId, PlaylistMusic>> {
    let list = conn.query::<(MusicId, String, Option<MusicDuration>, PlaylistId)>(
        r#"
        SELECT music.id, title, duration, playlist_id
        FROM music
        JOIN playlist_music ON music.id = playlist_music.music_id
    "#,
        [],
    )?;

    let list: Vec<PlaylistMusic> = list
        .into_iter()
        .map(|(id, title, duration, playlist_id)| PlaylistMusic {
            model: PlaylistMusicModel {
                playlist_id,
                music_id: id,
            },
            title,
            duration,
        })
        .collect();

    let musics: HashMap<MusicId, PlaylistMusic> = list
        .into_iter()
        .map(|music| {
            return (music.music_id(), music);
        })
        .collect();

    return Ok(musics);
}
