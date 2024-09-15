use ease_database::DbConnection;

use crate::ctx::Context;

pub fn get_conn(cx: &Context) -> anyhow::Result<DbConnection> {
    let conn = DbConnection::open(cx.db_uri.clone())?;
    Ok(conn)
}
