//! End-to-end migration test.
//!
//! Strategy:
//! 1. Copy the bundled `data.redb` fixture (a real v2-era dump) into a temp dir.
//! 2. Read every v3 row directly out of redb (after running the legacy
//!    v1->v2 and v2->v3 upgraders) to capture the *expected* post-migration
//!    state. These are the source-of-truth values.
//! 3. Invoke the public `migrate(document_dir)` entry point exactly as the
//!    backend does on startup.
//! 4. Read every row back out of SQLite via the public `*_to_model`
//!    converters and assert field-by-field equality with the redb source.
//! 5. Assert the legacy `data.redb` file has been deleted.

use std::path::PathBuf;
use std::sync::Arc;

use ease_client_migration::converter;
use ease_client_migration::legacy::{upgrade_v1_to_v2, upgrade_v2_to_v3};
use ease_client_migration::legacy::v3::{
    TABLE_BLOB, TABLE_ID_ALLOC, TABLE_MUSIC, TABLE_MUSIC_PLAYLIST, TABLE_PLAYLIST,
    TABLE_PREFERENCE, TABLE_STORAGE, TABLE_SCHEMA_VERSION,
};
use ease_client_migration::{migrate, SCHEMA_VERSION};
use ease_client_migration::entities::{
    blob, id_alloc, music, playlist, playlist_music, preference, schema_version, storage,
};
use ease_client_schema::v3;
use redb::{ReadableMultimapTable, ReadableTable};
use sea_orm::{EntityTrait, QueryOrder, PaginatorTrait};

/// Read every v3 row out of the redb fixture. Returns the full expected state.
struct ExpectedState {
    playlists: Vec<v3::PlaylistModel>,
    music: Vec<v3::MusicModel>,
    storage: Vec<v3::StorageModel>,
    /// (playlist_id, music_id) pairs from the music_playlist multimap.
    playlist_music: Vec<(v3::PlaylistId, v3::MusicId)>,
    preference: Option<v3::PreferenceModel>,
    /// (alloc_kind, next_id) pairs.
    id_alloc: Vec<(v3::DbKeyAlloc, i64)>,
    blob_next_id: Option<i64>,
    src_schema_version: u32,
}

fn read_expected_from_redb(db: &Arc<redb::Database>) -> ExpectedState {
    let src_version = {
        let rdb = db.begin_read().unwrap();
        match rdb.open_table(TABLE_SCHEMA_VERSION) {
            Ok(t) => t.get(()).unwrap().map(|v| v.value()).unwrap_or(0),
            Err(_) => 0,
        }
    };
    assert!(
        src_version >= 3,
        "fixture must be at v3 after upgraders; got {src_version}"
    );

    let playlists = {
        let rdb = db.begin_read().unwrap();
        let t = rdb.open_table(TABLE_PLAYLIST).unwrap();
        let mut v = Vec::new();
        for row in t.iter().unwrap() {
            let (_, val) = row.unwrap();
            v.push(val.value());
        }
        v
    };

    let music = {
        let rdb = db.begin_read().unwrap();
        let t = rdb.open_table(TABLE_MUSIC).unwrap();
        let mut v = Vec::new();
        for row in t.iter().unwrap() {
            let (_, val) = row.unwrap();
            v.push(val.value());
        }
        v
    };

    let storage = {
        let rdb = db.begin_read().unwrap();
        let t = rdb.open_table(TABLE_STORAGE).unwrap();
        let mut v = Vec::new();
        for row in t.iter().unwrap() {
            let (_, val) = row.unwrap();
            v.push(val.value());
        }
        v
    };

    let playlist_music = {
        let rdb = db.begin_read().unwrap();
        let t = rdb.open_multimap_table(TABLE_MUSIC_PLAYLIST).unwrap();
        let mut v = Vec::new();
        for entry in t.iter().unwrap() {
            let (mid, playlists) = entry.unwrap();
            let mid_val = mid.value();
            for p in playlists {
                let pid = p.unwrap().value();
                v.push((pid, mid_val));
            }
        }
        v
    };

    let preference = {
        let rdb = db.begin_read().unwrap();
        if let Ok(t) = rdb.open_table(TABLE_PREFERENCE) {
            if let Some(val) = t.get(()).unwrap() {
                Some(val.value())
            } else {
                None
            }
        } else {
            None
        }
    };

    let id_alloc = {
        let rdb = db.begin_read().unwrap();
        if let Ok(t) = rdb.open_table(TABLE_ID_ALLOC) {
            let mut v = Vec::new();
            for row in t.iter().unwrap() {
                let (k, val) = row.unwrap();
                v.push((k.value(), val.value()));
            }
            v
        } else {
            Vec::new()
        }
    };

    let blob_next_id = {
        let rdb = db.begin_read().unwrap();
        if let Ok(t) = rdb.open_table(TABLE_BLOB) {
            if let Some(val) = t.get(()).unwrap() {
                let bs: v3::BlobId = val.value();
                Some(*bs.as_ref())
            } else {
                None
            }
        } else {
            None
        }
    };

    ExpectedState {
        playlists,
        music,
        storage,
        playlist_music,
        preference,
        id_alloc,
        blob_next_id,
        src_schema_version: src_version,
    }
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

    // 2. Open the redb copy, run legacy upgraders, and snapshot the expected state.
    let redb_db = Arc::new(redb::Database::open(&redb_path).unwrap());
    upgrade_v1_to_v2(&redb_db).unwrap();
    upgrade_v2_to_v3(&redb_db).unwrap();
    let expected = read_expected_from_redb(&redb_db);
    // Drop the read handle so `migrate` can later delete the file.
    drop(redb_db);

    // The legacy schema in redb must be exactly v3 after the upgraders;
    // SQLite is bumped to v4.
    assert_eq!(expected.src_schema_version, 3);

    // Sanity: the fixture actually contains data worth checking.
    assert!(
        !expected.music.is_empty(),
        "fixture should contain at least one music row"
    );
    assert!(
        !expected.storage.is_empty(),
        "fixture should contain at least one storage row"
    );
    assert!(
        !expected.playlists.is_empty(),
        "fixture should contain at least one playlist row"
    );

    // 3. Run the public migration entry point.
    let db = migrate(&document_dir).await.unwrap();

    // 4. schema_version stamped at 4 in SQLite.
    let sv = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
        .one(&db)
        .await
        .unwrap()
        .expect("schema_version row missing");
    assert_eq!(sv.version, SCHEMA_VERSION);

    // 5. legacy redb file removed.
    assert!(
        !redb_path.exists(),
        "data.redb should have been deleted after successful import"
    );
    // And data.db exists in its place.
    assert!(dir.path().join("data.db").exists());

    // ---------- playlists ----------
    let playlist_rows: Vec<playlist::Model> = playlist::Entity::find()
        .order_by_asc(playlist::Column::Id)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        playlist_rows.len(),
        expected.playlists.len(),
        "playlist row count mismatch"
    );
    let mut exp_playlists = expected.playlists.clone();
    exp_playlists.sort_by_key(|p| *p.id.as_ref());
    for (row, exp) in playlist_rows.iter().zip(exp_playlists.iter()) {
        let got = converter::playlist_to_model(row.clone());
        assert_eq!(got.id, exp.id, "playlist id mismatch");
        assert_eq!(got.title, exp.title, "playlist title mismatch for id {:?}", exp.id);
        assert_eq!(got.created_time, exp.created_time);
        assert_eq!(got.picture, exp.picture, "playlist picture mismatch for id {:?}", exp.id);
        assert_eq!(got.order, exp.order, "playlist order mismatch for id {:?}", exp.id);
    }

    // ---------- music ----------
    let music_rows: Vec<music::Model> = music::Entity::find()
        .order_by_asc(music::Column::Id)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        music_rows.len(),
        expected.music.len(),
        "music row count mismatch"
    );
    let mut exp_music = expected.music.clone();
    exp_music.sort_by_key(|m| *m.id.as_ref());
    for (row, exp) in music_rows.iter().zip(exp_music.iter()) {
        let got = converter::music_to_model(row.clone());
        assert_eq!(got.id, exp.id, "music id mismatch");
        assert_eq!(got.loc, exp.loc, "music loc mismatch for id {:?}", exp.id);
        assert_eq!(got.title, exp.title, "music title mismatch for id {:?}", exp.id);
        assert_eq!(got.duration, exp.duration, "music duration mismatch for id {:?}", exp.id);
        assert_eq!(got.cover, exp.cover, "music cover mismatch for id {:?}", exp.id);
        assert_eq!(got.lyric, exp.lyric, "music lyric mismatch for id {:?}", exp.id);
        assert_eq!(got.lyric_default, exp.lyric_default);
        assert_eq!(got.order, exp.order, "music order mismatch for id {:?}", exp.id);
    }

    // ---------- storage ----------
    let storage_rows: Vec<storage::Model> = storage::Entity::find()
        .order_by_asc(storage::Column::Id)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        storage_rows.len(),
        expected.storage.len(),
        "storage row count mismatch"
    );
    let mut exp_storage = expected.storage.clone();
    exp_storage.sort_by_key(|s| *s.id.as_ref());
    for (row, exp) in storage_rows.iter().zip(exp_storage.iter()) {
        let got = converter::storage_to_model(row.clone());
        assert_eq!(got.id, exp.id);
        assert_eq!(got.addr, exp.addr, "storage addr mismatch for id {:?}", exp.id);
        assert_eq!(got.alias, exp.alias);
        assert_eq!(got.username, exp.username);
        assert_eq!(got.password, exp.password);
        assert_eq!(got.is_anonymous, exp.is_anonymous);
        assert_eq!(got.typ, exp.typ);
    }

    // ---------- playlist_music ----------
    let pm_rows: Vec<playlist_music::Model> = playlist_music::Entity::find()
        .order_by_asc(playlist_music::Column::PlaylistId)
        .order_by_asc(playlist_music::Column::MusicId)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        pm_rows.len(),
        expected.playlist_music.len(),
        "playlist_music row count mismatch"
    );
    let mut exp_pm = expected.playlist_music.clone();
    exp_pm.sort_by(|a, b| {
        a.0.cmp(&b.0).then(a.1.cmp(&b.1))
    });
    for (row, (pid, mid)) in pm_rows.iter().zip(exp_pm.iter()) {
        assert_eq!(row.playlist_id, *pid.as_ref(), "playlist_music.playlist_id mismatch");
        assert_eq!(row.music_id, *mid.as_ref(), "playlist_music.music_id mismatch");
    }

    // ---------- preference ----------
    let pref_row = preference::Entity::find()
        .one(&db)
        .await
        .unwrap();
    match (&expected.preference, &pref_row) {
        (Some(exp), Some(row)) => {
            let got = converter::preference_to_model(row.clone());
            assert_eq!(got.playmode, exp.playmode, "preference playmode mismatch");
        }
        (None, None) => {}
        (exp, got) => panic!("preference presence mismatch: expected {exp:?}, got {got:?}"),
    }

    // ---------- id_alloc ----------
    let id_alloc_rows: Vec<id_alloc::Model> = id_alloc::Entity::find()
        .order_by_asc(id_alloc::Column::Kind)
        .all(&db)
        .await
        .unwrap();
    assert_eq!(
        id_alloc_rows.len(),
        expected.id_alloc.len(),
        "id_alloc row count mismatch"
    );
    for (row, (kind, next_id)) in id_alloc_rows.iter().zip(expected.id_alloc.iter()) {
        let expected_kind = match kind {
            v3::DbKeyAlloc::Playlist => 0,
            v3::DbKeyAlloc::Music => 1,
            v3::DbKeyAlloc::Storage => 2,
        };
        assert_eq!(row.kind, expected_kind, "id_alloc kind mismatch");
        assert_eq!(row.next_id, *next_id, "id_alloc next_id mismatch for kind {kind:?}");
    }

    // ---------- blob next_id ----------
    let blob_row = blob::Entity::find_by_id(blob::Model::ROW_ID)
        .one(&db)
        .await
        .unwrap();
    match (expected.blob_next_id, blob_row) {
        (Some(exp), Some(row)) => {
            assert_eq!(row.next_id, exp, "blob next_id mismatch");
        }
        (None, None) => {}
        (exp, got) => panic!("blob presence mismatch: expected {exp:?}, got {got:?}"),
    }

    // ---------- idempotency: second migrate() must not duplicate rows ----------
    drop(db);
    let db2 = migrate(&document_dir).await.unwrap();
    assert_eq!(
        playlist::Entity::find().count(&db2).await.unwrap(),
        expected.playlists.len() as u64,
        "second migrate() must not duplicate playlist rows"
    );
    assert_eq!(
        music::Entity::find().count(&db2).await.unwrap(),
        expected.music.len() as u64,
        "second migrate() must not duplicate music rows"
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
