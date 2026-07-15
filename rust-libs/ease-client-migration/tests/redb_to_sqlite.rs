use std::path::PathBuf;

use ease_client_migration::{import_from_redb, open_database, entities::{music, playlist, storage, playlist_music, schema_version}};
use sea_orm::{EntityTrait, PaginatorTrait};
use sea_orm_migration::MigratorTrait;

#[tokio::test]
async fn import_redb_fixture_to_sqlite() {
    let dir = tempfile::tempdir().unwrap();

    // Open the SQLite target first and run the schema migrations.
    let db = open_database(dir.path().to_str().unwrap()).await.unwrap();
    ease_client_migration::Migrator::up(&db, None).await.unwrap();

    // Sanity: tables are present and empty before import.
    assert_eq!(playlist::Entity::find().count(&db).await.unwrap(), 0);
    assert_eq!(music::Entity::find().count(&db).await.unwrap(), 0);

    // Copy the bundled fixture to a temp path so we can open it read-only.
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data.redb");
    let tmp = dir.path().join("data.redb");
    std::fs::copy(&src, &tmp).unwrap();

    import_from_redb(&tmp, &db).await.unwrap();

    // After import, the SQLite schema_version should be 4.
    let row = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
        .one(&db)
        .await
        .unwrap()
        .expect("schema_version row must exist after import");
    assert_eq!(row.version, ease_client_migration::SCHEMA_VERSION);

    // At least one storage row should have made it across.
    let storage_count = storage::Entity::find().count(&db).await.unwrap();
    assert!(storage_count >= 1, "expected at least one storage row");

    // The join table should also be populated if the fixture had playlists.
    let _ = playlist_music::Entity::find().count(&db).await.unwrap();
}
