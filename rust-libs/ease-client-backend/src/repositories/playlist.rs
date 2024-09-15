use std::collections::HashMap;

use ease_database::{params, DbConnectionRef};

use crate::models::{
    music::MusicId,
    playlist::{PlaylistId, PlaylistModel, PlaylistMusicModel},
    storage::StorageId,
};

pub struct ArgDBUpsertPlaylist {
    pub id: Option<PlaylistId>,
    pub title: String,
    pub picture: Option<(StorageId, String)>,
}
pub fn db_upsert_playlist(
    conn: DbConnectionRef,
    arg: ArgDBUpsertPlaylist,
    current_time_ms: i64,
) -> anyhow::Result<PlaylistId> {
    let (picture_storage_id, picture_path) = if let Some(picture) = arg.picture {
        (Some(picture.0), Some(picture.1))
    } else {
        (None, None)
    };
    if let Some(id) = arg.id {
        conn.execute(
            "UPDATE playlist SET title = ?1, picture_storage_id = ?2, picture_path = ?3 WHERE id = ?4",
            params![arg.title, picture_storage_id, picture_path, id],
        )?;
        Ok(id)
    } else {
        let id = conn.query::<PlaylistId>(
            "INSERT INTO playlist (title, picture_storage_id, picture_path, created_time) VALUES (?1, ?2, ?3) RETURNING id",
            params![arg.title, picture_storage_id, picture_path, current_time_ms],
        )?.pop().unwrap();
        Ok(id)
    }
}

pub fn db_remove_playlist(conn: DbConnectionRef, id: PlaylistId) -> anyhow::Result<()> {
    conn.execute("DELETE FROM playlist WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn db_remove_all_musics_in_playlist(
    conn: DbConnectionRef,
    id: PlaylistId,
) -> anyhow::Result<()> {
    conn.execute(
        "DELETE FROM playlist_music WHERE playlist_id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn db_remove_music_from_playlist(
    conn: DbConnectionRef,
    playlist_id: PlaylistId,
    music_id: MusicId,
) -> anyhow::Result<()> {
    conn.execute(
        "DELETE FROM playlist_music WHERE playlist_id = ?1 AND music_id = ?2",
        params![playlist_id, music_id],
    )?;
    Ok(())
}

pub fn db_batch_add_music_to_playlist(
    conn: DbConnectionRef,
    args: Vec<(MusicId, PlaylistId)>,
) -> anyhow::Result<()> {
    for (music_id, playlist_id) in args {
        conn.execute(
            "INSERT OR IGNORE INTO playlist_music (playlist_id, music_id) VALUES (?1, ?2)",
            params![playlist_id, music_id],
        )?;
    }
    Ok(())
}

pub fn db_load_playlists(conn: DbConnectionRef) -> anyhow::Result<Vec<PlaylistModel>> {
    let playlist_models = conn.query::<PlaylistModel>(
        r#"
        SELECT id, title, picture, created_time FROM playlist;
    "#,
        [],
    )?;
    Ok(playlist_models)
}

fn db_load_playlists_impl(app: MistyClientHandle) -> anyhow::Result<HashMap<PlaylistId, Playlist>> {
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

pub type FirstMusicCovers = HashMap<PlaylistId, Option<Vec<u8>>>;

pub fn db_load_first_music_covers(conn: DbConnectionRef) -> anyhow::Result<FirstMusicCovers> {
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

    let map: FirstMusicCovers = list.into_iter().map(|v| (v.1, v.2)).collect();
    Ok(map)
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
