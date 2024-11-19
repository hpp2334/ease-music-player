pub use rusqlite::params;
pub use rusqlite::types::{
    FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, Value, ValueRef,
};
use serde::de::DeserializeOwned;

pub struct DbConnection {
    conn: rusqlite::Connection,
}

#[derive(Debug, Clone, Copy)]
pub struct DbConnectionRef<'a> {
    conn: &'a rusqlite::Connection,
}

pub type Result<T> = rusqlite::Result<T>;

pub type Error = rusqlite::Error;

impl DbConnection {
    pub fn open(path: String) -> Result<Self> {
        Ok(Self {
            conn: rusqlite::Connection::open(path)?,
        })
    }

    pub fn transaction<T, E>(
        &mut self,
        f: impl FnOnce(DbConnectionRef) -> std::result::Result<T, E>,
    ) -> std::result::Result<T, E>
    where
        E: From<Error>,
    {
        tracing::trace!("start db transaction");
        let transaction = self.conn.transaction()?;

        let conn = DbConnectionRef { conn: &transaction };

        let value = f(conn)?;
        transaction.commit()?;
        tracing::trace!("end db transaction");
        Ok(value)
    }

    pub fn get_ref(&self) -> DbConnectionRef<'_> {
        DbConnectionRef { conn: &self.conn }
    }

    pub fn query<T: DeserializeOwned>(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<T>> {
        tracing::trace!("start db query");
        let ret = self.get_ref().query(sql, params);
        tracing::trace!("end db query");
        ret
    }

    pub fn execute(&self, sql: &str, params: impl rusqlite::Params) -> Result<()> {
        tracing::trace!("start db execute");
        let ret = self.get_ref().execute(sql, params);
        tracing::trace!("end db execute");
        ret
    }

    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        tracing::trace!("start db execute_batch");
        let ret = self.get_ref().execute_batch(sql);
        tracing::trace!("end db execute_batch");
        ret
    }
}

impl<'a> DbConnectionRef<'a> {
    pub fn query<T: DeserializeOwned>(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<T>> {
        tracing::trace!("start dbref query");
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query(params)?;
        let list: Vec<T> = serde_rusqlite::from_rows::<T>(rows)
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        tracing::trace!("end dbref query");
        Ok(list)
    }

    pub fn execute(&self, sql: &str, params: impl rusqlite::Params) -> Result<()> {
        tracing::trace!("start dbref execute");
        self.conn.execute(sql, params)?;
        tracing::trace!("end dbref execute");
        Ok(())
    }

    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        tracing::trace!("start dbref execute_batch");
        self.conn.execute_batch(sql)?;
        tracing::trace!("end dbref execute_batch");
        Ok(())
    }
}
