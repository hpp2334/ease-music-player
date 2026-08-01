#![allow(dead_code)]

use ease_client_schema::{
    BlobId, DbKeyAlloc, MusicId, MusicModel, PlayMode, PlaylistId, PlaylistModel, PreferenceModel,
    StorageEntryLoc, StorageId,
};

use ease_client_schema::entities::{blob, id_alloc, music, playlist, playlist_music, preference};

const FALSE_I32: i32 = 0;
const TRUE_I32: i32 = 1;

fn db_key_alloc_index(k: &DbKeyAlloc) -> i32 {
    match k {
        DbKeyAlloc::Playlist => 0,
        DbKeyAlloc::Music => 1,
        DbKeyAlloc::Storage => 2,
    }
}

fn play_mode_index(p: PlayMode) -> i32 {
    match p {
        PlayMode::Single => 0,
        PlayMode::SingleLoop => 1,
        PlayMode::List => 2,
        PlayMode::ListLoop => 3,
    }
}

fn play_mode_from_index(i: i32) -> PlayMode {
    match i {
        0 => PlayMode::Single,
        1 => PlayMode::SingleLoop,
        2 => PlayMode::List,
        3 => PlayMode::ListLoop,
        _ => PlayMode::default(),
    }
}

fn encode_order(order: &[u32]) -> String {
    serde_json::to_string(order).unwrap_or_else(|_| "[]".to_string())
}

pub fn decode_order(s: &str) -> Vec<u32> {
    serde_json::from_str(s).unwrap_or_default()
}

pub fn id_alloc_from(kind: DbKeyAlloc, next_id: i64) -> id_alloc::ActiveModel {
    id_alloc::ActiveModel {
        kind: sea_orm::ActiveValue::Set(db_key_alloc_index(&kind)),
        next_id: sea_orm::ActiveValue::Set(next_id),
    }
}

pub fn playlist_from(m: PlaylistModel) -> playlist::ActiveModel {
    playlist::ActiveModel {
        id: sea_orm::ActiveValue::Set(*m.id.as_ref()),
        title: sea_orm::ActiveValue::Set(m.title),
        created_time: sea_orm::ActiveValue::Set(m.created_time),
        picture_storage_id: sea_orm::ActiveValue::Set(m.picture.as_ref().map(|p| *p.storage_id.as_ref())),
        picture_path: sea_orm::ActiveValue::Set(m.picture.map(|p| p.path)),
        order: sea_orm::ActiveValue::Set(encode_order(&m.order)),
    }
}

pub fn playlist_to_model(row: playlist::Model) -> PlaylistModel {
    PlaylistModel {
        id: PlaylistId::wrap(row.id),
        title: row.title,
        created_time: row.created_time,
        picture: match (row.picture_storage_id, row.picture_path) {
            (Some(sid), Some(path)) => Some(StorageEntryLoc {
                storage_id: StorageId::wrap(sid),
                path,
            }),
            _ => None,
        },
        order: decode_order(&row.order),
    }
}

pub fn music_from(m: MusicModel) -> music::ActiveModel {
    music::ActiveModel {
        id: sea_orm::ActiveValue::Set(*m.id.as_ref()),
        loc_storage_id: sea_orm::ActiveValue::Set(*m.loc.storage_id.as_ref()),
        loc_path: sea_orm::ActiveValue::Set(m.loc.path),
        title: sea_orm::ActiveValue::Set(m.title),
        duration_ms: sea_orm::ActiveValue::Set(m.duration.map(|d| d.as_millis() as i64)),
        cover_blob_id: sea_orm::ActiveValue::Set(m.cover.map(|c| *c.as_ref())),
        lyric_storage_id: sea_orm::ActiveValue::Set(m.lyric.as_ref().map(|l| *l.storage_id.as_ref())),
        lyric_path: sea_orm::ActiveValue::Set(m.lyric.map(|l| l.path)),
        lyric_default: sea_orm::ActiveValue::Set(if m.lyric_default { TRUE_I32 } else { FALSE_I32 }),
        order: sea_orm::ActiveValue::Set(encode_order(&m.order)),
    }
}

pub fn music_to_model(row: music::Model) -> MusicModel {
    MusicModel {
        id: MusicId::wrap(row.id),
        loc: StorageEntryLoc {
            storage_id: StorageId::wrap(row.loc_storage_id),
            path: row.loc_path,
        },
        title: row.title,
        duration: row.duration_ms.map(|ms| std::time::Duration::from_millis(ms as u64)),
        cover: row.cover_blob_id.map(BlobId::wrap),
        lyric: match (row.lyric_storage_id, row.lyric_path) {
            (Some(sid), Some(path)) => Some(StorageEntryLoc {
                storage_id: StorageId::wrap(sid),
                path,
            }),
            _ => None,
        },
        lyric_default: row.lyric_default != FALSE_I32,
        order: decode_order(&row.order),
    }
}

pub fn playlist_music_from(playlist_id: PlaylistId, music_id: MusicId) -> playlist_music::ActiveModel {
    playlist_music::ActiveModel {
        playlist_id: sea_orm::ActiveValue::Set(*playlist_id.as_ref()),
        music_id: sea_orm::ActiveValue::Set(*music_id.as_ref()),
    }
}

pub fn preference_from(m: PreferenceModel) -> preference::ActiveModel {
    preference::ActiveModel {
        id: sea_orm::ActiveValue::Set(0),
        playmode: sea_orm::ActiveValue::Set(play_mode_index(m.playmode)),
    }
}

pub fn preference_to_model(row: preference::Model) -> PreferenceModel {
    PreferenceModel {
        playmode: play_mode_from_index(row.playmode),
    }
}

pub fn blob_alloc_from(next_id: i64) -> blob::ActiveModel {
    blob::ActiveModel {
        id: sea_orm::ActiveValue::Set(blob::Model::ROW_ID),
        next_id: sea_orm::ActiveValue::Set(next_id),
    }
}
