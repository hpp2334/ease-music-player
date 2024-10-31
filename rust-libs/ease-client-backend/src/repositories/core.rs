use ease_database::DbConnection;

use crate::{ctx::BackendContext, error::BResult};

pub fn get_conn(cx: &BackendContext) -> BResult<DbConnection> {
    let db_uri = cx.get_app_document_dir() + "app.db";
    let conn = DbConnection::open(db_uri)?;
    Ok(conn)
}
