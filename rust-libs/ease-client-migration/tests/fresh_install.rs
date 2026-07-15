use ease_client_migration::{migrate, SCHEMA_VERSION};
use ease_client_migration::entities::schema_version;
use sea_orm::EntityTrait;

/// No legacy redb present: migrate() should create the SQLite file, run the
/// schema migrations, and stamp schema_version = SCHEMA_VERSION.
#[tokio::test]
async fn fresh_install_stamps_schema_version() {
    let dir = tempfile::tempdir().unwrap();

    let db = migrate(dir.path().to_str().unwrap()).await.unwrap();

    let row = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
        .one(&db)
        .await
        .unwrap()
        .expect("schema_version row must be stamped on fresh install");
    assert_eq!(row.version, SCHEMA_VERSION);

    // The data.db file should exist on disk.
    assert!(dir.path().join("data.db").exists());

    // And no data.redb should have been created.
    assert!(!dir.path().join("data.redb").exists());
}

/// Calling migrate() twice in a row should be a no-op on the second call
/// (schema_version already at SCHEMA_VERSION, no legacy file to import).
#[tokio::test]
async fn migrate_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let dir_str = dir.path().to_str().unwrap().to_string();

    let _ = migrate(&dir_str).await.unwrap();
    let db = migrate(&dir_str).await.unwrap();

    let row = schema_version::Entity::find_by_id(schema_version::Model::ROW_ID)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.version, SCHEMA_VERSION);
}
