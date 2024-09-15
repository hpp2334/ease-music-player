use std::time::Duration;

use bytes::Bytes;
use ease_client_shared::{MusicDuration, MusicId, PlaylistId, StorageId};
use ease_database::{params, DbConnectionRef};

use crate::models::{music::MusicModel, storage::StorageEntryLocModel};

pub fn db_load_music_metas_by_playlist_id(
    conn: DbConnectionRef,
    playlist_id: PlaylistId,
) -> anyhow::Result<Vec<MusicModel>> {
    let models = conn.query::<MusicModel>(
        r#"
    SELECT id, title, duration
    FROM music
    WHERE id IN (SELECT music_id IN playlist_music WHERE playlist_id = ?1)
    "#,
        [playlist_id],
    )?;
    Ok(models)
}

pub fn db_load_music(
    conn: DbConnectionRef,
    music_id: MusicId,
) -> anyhow::Result<Option<MusicModel>> {
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
) -> anyhow::Result<Option<MusicModel>> {
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

pub fn db_add_music(conn: DbConnectionRef, arg: ArgDBAddMusic) -> anyhow::Result<MusicId> {
    let music = db_load_music_by_key(conn, arg.storage_id.clone(), arg.path.clone())?;
    if let Some(music) = music {
        return Ok(music.id);
    }

    let inserted_id = conn
        .query::<MusicId>(
            r#"
        INSERT INTO music (storage_id, path, title)
        VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id"#,
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
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE music set duration = ?1 WHERE id = ?2",
        params![duration, id],
    )?;

    Ok(())
}

pub fn db_update_music_cover(
    conn: DbConnectionRef,
    id: MusicId,
    cover_loc: StorageEntryLocModel,
) -> ease_database::Result<()> {
    conn.execute(
        "UPDATE music set picture_path = ?1, picture_storage_id = ?2 WHERE id = ?3",
        params![cover_loc.0, cover_loc.1, id],
    )?;

    Ok(())
}

pub fn db_update_music_lyric(
    conn: DbConnectionRef,
    id: MusicId,
    lyric_loc: StorageEntryLocModel,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE music set lyric_storage_id = ?2, lyric_path = ?3 WHERE id = ?1",
        params![id, lyric_loc.1, lyric_loc.0],
    )?;
    Ok(())
}
