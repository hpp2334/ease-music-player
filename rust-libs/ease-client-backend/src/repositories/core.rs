use ease_database::DbConnection;

use crate::{ctx::Context, error::BResult};

pub fn get_conn(cx: &Context) -> BResult<DbConnection> {
    let db_uri = cx.storage_path.clone() + "app.db";
    let conn = DbConnection::open(db_uri)?;
    Ok(conn)
}
