use ease_database::DbConnection;

use crate::ctx::Context;

pub fn get_conn(cx: &Context) -> anyhow::Result<DbConnection> {
    let db_uri = cx.storage_path.clone() + "app.db";
    let conn = DbConnection::open(db_uri)?;
    Ok(conn)
}
