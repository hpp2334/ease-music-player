use ease_database::DbConnection;

use crate::{ctx::BackendContext, error::BResult};

pub fn get_conn(cx: &BackendContext) -> BResult<DbConnection> {
    let db_uri = cx.get_storage_path() + "app.db";
    let conn = DbConnection::open(db_uri)?;
    Ok(conn)
}
