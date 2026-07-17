//! End-to-end migration test (black-box).
//!
//! Treats `ease-client-migration` purely as its public API: drop the bundled
//! `data.redb` fixture in place, call `migrate(document_dir)`, then verify
//! the resulting SQLite contents. Expected values are cached from a known-good
//! migration run (captured 2026-07-18 against the committed `data.redb`).

use std::path::PathBuf;

use ease_client_migration::{migrate, SCHEMA_VERSION};
use ease_client_schema::entities::{
    blob, id_alloc, music, playlist, playlist_music, preference, schema_version, storage,
};
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder};

async fn dump<C: sea_orm::ConnectionTrait>(db: &C) -> (
    u32,
    Vec<id_alloc::Model>,
    Vec<playlist::Model>,
    Vec<music::Model>,
    Vec<storage::Model>,
    Vec<playlist_music::Model>,
    Option<preference::Model>,
    Option<blob::Model>,
) {
    let sv = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
        .one(db)
        .await
        .unwrap()
        .map(|r| r.version)
        .unwrap_or(0);

    let allocs = id_alloc::Entity::find()
        .order_by_asc(id_alloc::Column::Kind)
        .all(db)
        .await
        .unwrap();
    let playlists = playlist::Entity::find()
        .order_by_asc(playlist::Column::Id)
        .all(db)
        .await
        .unwrap();
    let music_rows = music::Entity::find()
        .order_by_asc(music::Column::Id)
        .all(db)
        .await
        .unwrap();
    let storage_rows = storage::Entity::find()
        .order_by_asc(storage::Column::Id)
        .all(db)
        .await
        .unwrap();
    let pm_rows = playlist_music::Entity::find()
        .order_by_asc(playlist_music::Column::PlaylistId)
        .order_by_asc(playlist_music::Column::MusicId)
        .all(db)
        .await
        .unwrap();
    let pref = preference::Entity::find().one(db).await.unwrap();
    let blob_row = blob::Entity::find_by_id(blob::Model::ROW_ID)
        .one(db)
        .await
        .unwrap();

    (sv, allocs, playlists, music_rows, storage_rows, pm_rows, pref, blob_row)
}

#[tokio::test]
async fn e2e_migration_preserves_all_data() {
    let dir = tempfile::tempdir().unwrap();
    let document_dir = dir.path().to_str().unwrap().to_string();

    // 1. Drop the bundled fixture in place as `data.redb`.
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data.redb");
    let redb_path = dir.path().join("data.redb");
    std::fs::copy(&src, &redb_path).unwrap();

    // 2. Run the public migration entry point.
    let db = migrate(&document_dir).await.unwrap();

    // 3. Snapshot SQLite state.
    let (sv, allocs, playlists, music_rows, storage_rows, pm_rows, pref, blob_row) = dump(&db).await;

    // ---------- schema_version stamped at SCHEMA_VERSION ----------
    assert_eq!(sv, SCHEMA_VERSION);

    // ---------- legacy redb file removed; SQLite db in its place ----------
    assert!(!redb_path.exists(), "data.redb should have been deleted");
    assert!(dir.path().join("data.db").exists());

    // ---------- counts ----------
    assert_eq!(playlists.len(), 3, "playlist count");
    assert_eq!(music_rows.len(), 18, "music count");
    assert_eq!(storage_rows.len(), 2, "storage count");
    assert_eq!(pm_rows.len(), 18, "playlist_music count");
    assert_eq!(allocs.len(), 3, "id_alloc count");

    // ---------- blob allocator not present in this fixture ----------
    assert!(blob_row.is_none(), "blob row should be absent");

    // ---------- preference absent in this fixture ----------
    assert!(pref.is_none(), "preference row should be absent");

    // ---------- id_alloc rows ----------
    assert_eq!(allocs[0].kind, 0, "id_alloc[0] kind");
    assert_eq!(allocs[0].next_id, 3, "id_alloc[0] next_id");
    assert_eq!(allocs[1].kind, 1, "id_alloc[1] kind");
    assert_eq!(allocs[1].next_id, 18, "id_alloc[1] next_id");
    assert_eq!(allocs[2].kind, 2, "id_alloc[2] kind");
    assert_eq!(allocs[2].next_id, 2, "id_alloc[2] next_id");

    // ---------- storage rows ----------
    assert_eq!(storage_rows[0].id, 1);
    assert_eq!(storage_rows[0].addr, "");
    assert_eq!(storage_rows[0].alias, "Local");
    assert_eq!(storage_rows[0].username, "");
    assert_eq!(storage_rows[0].password, "");
    assert_eq!(storage_rows[0].is_anonymous, 0);
    assert_eq!(storage_rows[0].typ, 0, "storage[0] typ (Local)");

    assert_eq!(storage_rows[1].id, 2);
    assert_eq!(storage_rows[1].addr, "http://0.0.0.0:81");
    assert_eq!(storage_rows[1].alias, "A");
    assert_eq!(storage_rows[1].username, "admin");
    assert_eq!(storage_rows[1].password, "123456");
    assert_eq!(storage_rows[1].is_anonymous, 0);
    assert_eq!(storage_rows[1].typ, 1, "storage[1] typ (Webdav)");

    // ---------- playlist rows ----------
    assert_eq!(playlists[0].id, 1);
    assert_eq!(playlists[0].title, "雑踏、僕らの街");
    assert_eq!(playlists[0].created_time, 1756637463597);
    assert_eq!(playlists[0].picture_storage_id, Some(2));
    assert!(playlists[0].picture_path.is_some());
    assert_eq!(playlists[0].order, "[1]");

    assert_eq!(playlists[1].id, 2);
    assert_eq!(playlists[1].title, "樱之刻");
    assert_eq!(playlists[1].created_time, 1756637472444);
    assert_eq!(playlists[1].picture_storage_id, None);
    assert_eq!(playlists[1].picture_path, None);
    assert_eq!(playlists[1].order, "[2]");

    assert_eq!(playlists[2].id, 3);
    assert_eq!(playlists[2].title, "Download");
    assert_eq!(playlists[2].created_time, 1756637506647);
    assert_eq!(playlists[2].order, "[3]");

    // ---------- music spot-checks (first + last) ----------
    assert_eq!(music_rows[0].id, 1);
    assert_eq!(music_rows[0].title, "01 雑踏、僕らの街");
    assert_eq!(music_rows[0].duration_ms, Some(185066));
    assert_eq!(music_rows[0].lyric_default, 1);
    assert_eq!(music_rows[0].order, "[1]");

    assert_eq!(music_rows[17].id, 18);
    assert_eq!(music_rows[17].loc_storage_id, 1);
    assert_eq!(music_rows[17].title, "02 運命の華");
    assert_eq!(music_rows[17].duration_ms, Some(198533));
    assert_eq!(music_rows[17].order, "[18]");

    // Every music row landed in storage 2 except the last (Download, storage 1).
    for m in &music_rows[..17] {
        assert_eq!(m.loc_storage_id, 2, "music {} should be in storage 2", m.id);
        assert!(m.cover_blob_id.is_none());
        assert!(m.lyric_storage_id.is_none());
    }

    // ---------- playlist_music edges ----------
    // Playlist 1 -> music 1..5
    for (i, pm) in pm_rows.iter().enumerate().take(5) {
        assert_eq!(pm.playlist_id, 1, "pm[{}] playlist_id", i);
        assert_eq!(pm.music_id, (i + 1) as i64, "pm[{}] music_id", i);
    }
    // Playlist 2 -> music 6..17
    for (i, pm) in pm_rows.iter().enumerate().skip(5).take(12) {
        assert_eq!(pm.playlist_id, 2, "pm[{}] playlist_id", i);
        assert_eq!(pm.music_id, (i + 1) as i64, "pm[{}] music_id", i);
    }
    // Playlist 3 -> music 18
    assert_eq!(pm_rows[17].playlist_id, 3);
    assert_eq!(pm_rows[17].music_id, 18);

    // ---------- idempotency: second migrate() must not duplicate rows ----------
    drop(db);
    let db2 = migrate(&document_dir).await.unwrap();
    assert_eq!(
        playlist::Entity::find().count(&db2).await.unwrap(),
        3,
        "second migrate() must not duplicate playlist rows"
    );
    assert_eq!(
        music::Entity::find().count(&db2).await.unwrap(),
        18,
        "second migrate() must not duplicate music rows"
    );
    assert_eq!(
        playlist_music::Entity::find().count(&db2).await.unwrap(),
        18,
        "second migrate() must not duplicate playlist_music rows"
    );
}

#[tokio::test]
async fn fresh_install_stamps_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let document_dir = dir.path().to_str().unwrap().to_string();

    // No data.redb present -> fresh install path.
    let db = migrate(&document_dir).await.unwrap();

    let sv = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
        .one(&db)
        .await
        .unwrap()
        .expect("schema_version row must exist on fresh install");
    assert_eq!(sv.version, SCHEMA_VERSION);

    // No redb file should have been created.
    assert!(!dir.path().join("data.redb").exists());
    assert!(dir.path().join("data.db").exists());

    // All user tables empty.
    assert_eq!(music::Entity::find().count(&db).await.unwrap(), 0);
    assert_eq!(playlist::Entity::find().count(&db).await.unwrap(), 0);
    assert_eq!(storage::Entity::find().count(&db).await.unwrap(), 0);
}
