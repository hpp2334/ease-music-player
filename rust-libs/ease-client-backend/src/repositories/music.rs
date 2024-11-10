use ease_client_shared::backends::{
    music::MusicId, music_duration::MusicDuration, playlist::PlaylistId, storage::StorageId,
};
use ease_database::{params, DbConnectionRef};

use crate::{
    error::BResult,
    models::{music::MusicModel, storage::StorageEntryLocModel},
};

pub fn db_load_music_metas_by_playlist_id(
    conn: DbConnectionRef,
    playlist_id: PlaylistId,
) -> BResult<Vec<MusicModel>> {
    let models = conn.query::<MusicModel>(
        r#"
    SELECT *
    FROM music
    WHERE id IN (SELECT music_id FROM playlist_music WHERE playlist_id = ?1)
    "#,
        [playlist_id],
    )?;
    Ok(models)
}

pub fn db_load_music(conn: DbConnectionRef, music_id: MusicId) -> BResult<Option<MusicModel>> {
    let model = conn
        .query::<MusicModel>(
            r#"
        SELECT * FROM music WHERE id = ?1
    "#,
            params![music_id],
        )?
        .pop();

    Ok(model)
}

fn db_load_music_by_key(
    conn: DbConnectionRef,
    storage_id: StorageId,
    path: String,
) -> BResult<Option<MusicModel>> {
    let model = conn
        .query::<MusicModel>(
            "SELECT * FROM music WHERE storage_id = ?1 AND path = ?2",
            params![storage_id, path],
        )?
        .pop();

    Ok(model)
}

#[derive(Debug)]
pub struct ArgDBAddMusic {
    pub storage_id: StorageId,
    pub path: String,
    pub title: String,
}

pub fn db_add_music(conn: DbConnectionRef, arg: ArgDBAddMusic) -> BResult<MusicId> {
    let music = db_load_music_by_key(conn, arg.storage_id.clone(), arg.path.clone())?;
    if let Some(music) = music {
        return Ok(music.id);
    }

    let inserted_id = conn
        .query::<MusicId>(
            r#"
        INSERT INTO music (storage_id, path, title, lyric_default)
        VALUES (?1, ?2, ?3, true) RETURNING id"#,
            params![arg.storage_id.as_ref(), arg.path, arg.title,],
        )?
        .pop()
        .unwrap();

    return Ok(inserted_id);
}

pub fn db_update_music_total_duration(
    conn: DbConnectionRef,
    id: MusicId,
    duration: MusicDuration,
) -> BResult<()> {
    conn.execute(
        "UPDATE music set duration = ?1 WHERE id = ?2",
        params![duration, id],
    )?;

    Ok(())
}

pub fn db_update_music_cover(conn: DbConnectionRef, id: MusicId, cover: Vec<u8>) -> BResult<()> {
    conn.execute(
        "UPDATE music set cover = ?1 WHERE id = ?2",
        params![cover, id],
    )?;

    Ok(())
}

pub fn db_update_music_lyric(
    conn: DbConnectionRef,
    id: MusicId,
    lyric_loc: StorageEntryLocModel,
) -> BResult<()> {
    conn.execute(
        "UPDATE music set lyric_storage_id = ?2, lyric_path = ?3, lyric_default = false WHERE id = ?1",
        params![id, lyric_loc.1, lyric_loc.0],
    )?;
    Ok(())
}

pub fn db_get_playlists_count_by_storage(
    conn: DbConnectionRef,
    storage_id: StorageId,
) -> BResult<u32> {
    let mut list = conn.query::<u32>(
        r#"
        SELECT COUNT(DISTINCT p.id) AS playlist_count
FROM playlist p
JOIN playlist_music pm ON p.id = pm.playlist_id
JOIN music m ON pm.music_id = m.id
WHERE m.storage_id = ?;
    "#,
        [storage_id],
    )?;
    Ok(list.pop().unwrap())
}
