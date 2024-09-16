use std::collections::HashMap;

use ease_client_shared::backends::{music::MusicId, playlist::PlaylistId, storage::StorageId};
use ease_database::{params, DbConnectionRef};

use crate::models::{playlist::PlaylistModel, storage::StorageEntryLocModel};

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

pub type FirstMusicCovers = HashMap<PlaylistId, StorageEntryLocModel>;

pub fn db_load_first_music_covers(conn: DbConnectionRef) -> anyhow::Result<FirstMusicCovers> {
    let list = conn.query::<(MusicId, PlaylistId, Option<String>, Option<StorageId>)>(
        r#"
    SELECT music_id, playlist_id, picture_path, picture_storage_id
    FROM playlist_music
    JOIN music ON music.id = playlist_music.music_id
    WHERE music.picture_path NOT NULL
    GROUP BY playlist_id;
"#,
        [],
    )?;

    let map: FirstMusicCovers = list.into_iter().map(|v| (v.1, (v.2, v.3))).collect();
    Ok(map)
}

pub fn db_remove_musics_in_playlists_by_storage(
    conn: DbConnectionRef,
    storage_id: StorageId,
) -> anyhow::Result<()> {
    conn.execute(
        r#"DELETE FROM playlist_music
    WHERE id IN (SELECT id FROM music WHERE storage_id = ?1)"#,
        params![storage_id],
    )?;
    Ok(())
}
