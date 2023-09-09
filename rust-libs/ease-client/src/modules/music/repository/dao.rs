use std::time::Duration;

use bytes::Bytes;
use ease_database::{params, DbConnectionRef};
use misty_vm::client::MistyClientHandle;

use crate::modules::{app::service::get_db_conn_v2, error::EaseResult, StorageId};

use super::{
    super::{Music, MusicId, MusicMeta},
    MusicDuration, MusicModel,
};

fn build_music(client: MistyClientHandle, mut model: MusicModel) -> Music {
    let handle = if let Some(buf) = model.picture.take() {
        Some(client.resource_manager().insert(buf))
    } else {
        None
    };

    Music {
        model,
        picture: handle,
    }
}

pub fn db_load_music(client: MistyClientHandle, music_id: MusicId) -> EaseResult<Option<Music>> {
    let conn = get_db_conn_v2(client)?;
    let models = conn
        .query::<MusicModel>(
            r#"
        SELECT * FROM music WHERE id = ?1
    "#,
            params![music_id],
        )?
        .pop();

    Ok(models.map(|model| build_music(client, model)))
}

pub fn db_load_music_picture(
    client: MistyClientHandle,
    music_id: MusicId,
) -> EaseResult<Option<Bytes>> {
    let conn = get_db_conn_v2(client)?;

    let pic_info = conn
        .query::<Option<Vec<u8>>>("SELECT picture FROM music WHERE id = ?1", params![music_id])?
        .pop()
        .unwrap_or_default()
        .map(|buf| Bytes::from(buf));

    Ok(pic_info)
}

fn db_load_music_by_key(
    client: MistyClientHandle,
    conn: DbConnectionRef,
    storage_id: StorageId,
    path: String,
) -> EaseResult<Option<Music>> {
    let model = conn
        .query::<MusicModel>(
            "SELECT * FROM music WHERE storage_id = ?1 AND path = ?2",
            params![storage_id, path],
        )?
        .pop();

    Ok(model.map(|model| build_music(client, model)))
}

#[derive(Debug)]
pub struct ArgDBAddMusic {
    pub storage_id: StorageId,
    pub path: String,
    pub title: String,
}

fn get_default_lyric_path(mut p: String) -> String {
    let i = p.rfind('.').unwrap();
    p.replace_range(i.., ".lrc");
    p
}

pub fn db_add_music(client: MistyClientHandle, arg: ArgDBAddMusic) -> EaseResult<Music> {
    let conn = get_db_conn_v2(client)?;
    let music = db_load_music_by_key(
        client,
        conn.get_ref(),
        arg.storage_id.clone(),
        arg.path.clone(),
    )?;
    if let Some(music) = music {
        return Ok(music);
    }

    let lyric_storage_id = arg.storage_id;
    let lyric_path = get_default_lyric_path(arg.path.clone());

    let inserted_id = conn
        .query::<MusicId>(
            r#"
        INSERT INTO music (storage_id, path, title, lyric_storage_id, lyric_path)
        VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id"#,
            params![
                arg.storage_id.as_ref(),
                arg.path,
                arg.title,
                lyric_storage_id.as_ref(),
                lyric_path
            ],
        )?
        .pop()
        .unwrap();

    return Ok(db_load_music(client, inserted_id)?.unwrap());
}

pub fn db_update_music_total_duration(
    client: MistyClientHandle,
    id: MusicId,
    duration: Duration,
) -> EaseResult<()> {
    let conn = get_db_conn_v2(client)?;
    let duration = MusicDuration::new(duration);

    conn.execute(
        "UPDATE music set duration = ?1 WHERE id = ?2",
        params![duration, id],
    )?;

    Ok(())
}

fn db_update_music_picture_impl(
    conn: DbConnectionRef,
    id: MusicId,
    metadata: &Option<MusicMeta>,
) -> ease_database::Result<()> {
    let buf = metadata
        .as_ref()
        .map(|m| m.buf.clone())
        .unwrap_or_default()
        .map(|buf| buf.to_vec());

    conn.execute(
        "UPDATE music set picture = ?2 WHERE id = ?1",
        params![id, buf],
    )?;

    Ok(())
}

fn db_update_music_duration_by_metadata_impl(
    conn: DbConnectionRef,
    id: MusicId,
    metadata: &Option<MusicMeta>,
) -> ease_database::Result<()> {
    let duration = metadata.as_ref().map(|m| m.duration).unwrap_or_default();
    if let Some(duration) = duration {
        conn.execute(
            "UPDATE music set duration = ?2 WHERE id = ?1",
            params![id.as_ref(), MusicDuration::new(duration)],
        )?;
    }
    Ok(())
}

pub fn db_update_music_picture(
    client: MistyClientHandle,
    id: MusicId,
    metadata: &Option<MusicMeta>,
) -> EaseResult<()> {
    let conn = get_db_conn_v2(client)?;
    db_update_music_picture_impl(conn.get_ref(), id, metadata)?;
    Ok(())
}

pub fn db_update_music_picture_and_duration(
    client: MistyClientHandle,
    id: MusicId,
    metadata: &Option<MusicMeta>,
) -> EaseResult<()> {
    let mut conn = get_db_conn_v2(client)?;
    conn.transaction(|conn| {
        db_update_music_picture_impl(conn, id.clone(), &metadata)?;
        db_update_music_duration_by_metadata_impl(conn, id.clone(), &metadata)
    })?;
    Ok(())
}

pub(in crate::modules::music) fn db_update_music_lyric(
    client: MistyClientHandle,
    id: MusicId,
    lyric_storage_id: StorageId,
    lyric_path: String,
) -> EaseResult<()> {
    let conn = get_db_conn_v2(client)?;
    conn.execute(
        "UPDATE music set lyric_storage_id = ?2, lyric_path = ?3 WHERE id = ?1",
        params![id, lyric_storage_id, lyric_path],
    )?;
    Ok(())
}

pub(in crate::modules::music) fn db_remove_music_lyric(
    client: MistyClientHandle,
    id: MusicId,
) -> EaseResult<()> {
    let conn = get_db_conn_v2(client)?;

    conn.execute(
        "UPDATE music set lyric_storage_id = ?2, lyric_path = ?3 WHERE id = ?1",
        params![id.as_ref(), None::<i64>, None::<String>],
    )?;

    Ok(())
}
