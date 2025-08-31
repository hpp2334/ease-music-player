use std::{path::PathBuf, sync::Arc};

use ease_client_schema::upgrade_v2_to_v3;

#[test]
fn test_v2_to_v3() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(file!())
        .parent()
        .unwrap()
        .join("data.redb");
    let db = redb::Database::open(p).unwrap();
    let db = Arc::new(db);
    upgrade_v2_to_v3(&db).unwrap();
    assert!(true);
}
