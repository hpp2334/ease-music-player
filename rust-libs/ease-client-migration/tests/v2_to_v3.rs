use std::{path::PathBuf, sync::Arc};

use ease_client_migration::legacy::upgrade_v2_to_v3;

#[test]
fn test_v2_to_v3() {
    // Re-use the bundled fixture (originally a v2 redb) and copy it into a
    // temp location so the test never mutates the checked-in fixture.
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data.redb");
    let tmp = tempfile::NamedTempFile::new().unwrap().into_temp_path();
    std::fs::copy(&src, &tmp).unwrap();

    let db = redb::Database::open(&tmp).unwrap();
    let db = Arc::new(db);
    upgrade_v2_to_v3(&db).unwrap();
}
