use ease_database::DbConnection;

use crate::{ctx::BackendGlobal, error::BResult};

pub fn get_conn(cx: &BackendGlobal) -> BResult<DbConnection> {
    let db_uri = cx.storage_path.clone() + "app.db";
    let conn = DbConnection::open(db_uri)?;
    Ok(conn)
}
