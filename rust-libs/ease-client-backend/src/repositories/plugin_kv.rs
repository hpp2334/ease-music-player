use std::sync::Arc;

use ease_client_schema::entities::{plugin_kv_key, plugin_kv_multi, plugin_kv_single};
use ease_client_schema::{
    PluginKvCountEntry, PluginKvEntry, PluginKvKeyInfo, PluginKvKind, PluginKvMultiEntry,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, FromQueryResult, Order,
    PaginatorTrait, QueryFilter, QueryOrder, Set, Statement,
};

use crate::error::{BError, BResult};

use super::core::DatabaseServer;

impl DatabaseServer {
    // -----------------------------------------------------------------------
    // Key-registry helpers
    // -----------------------------------------------------------------------

    /// Resolve or register the integer `key_id` for `(plugin_id, key)`,
    /// locking it to `kind`. If the key already exists with a different
    /// kind, returns a kind-mismatch error.
    async fn resolve_key_id(
        self: &Arc<Self>,
        plugin_id: &str,
        key: &str,
        kind: PluginKvKind,
    ) -> BResult<i64> {
        let db = self.db();
        if let Some(row) = plugin_kv_key::Entity::find()
            .filter(plugin_kv_key::Column::PluginId.eq(plugin_id))
            .filter(plugin_kv_key::Column::Key.eq(key))
            .one(&db)
            .await?
        {
            if row.kind != kind.as_i32() {
                return Err(BError::CustomError {
                    message: format!(
                        "plugin_kv: key kind mismatch for plugin={:?} key={:?} (existing={}, requested={})",
                        plugin_id, key, row.kind, kind.as_i32()
                    ),
                });
            }
            return Ok(row.id);
        }
        let now = now_ms();
        let am = plugin_kv_key::ActiveModel {
            plugin_id: Set(plugin_id.to_string()),
            key: Set(key.to_string()),
            kind: Set(kind.as_i32()),
            created_at: Set(now),
            ..Default::default()
        };
        let row = am.insert(&db).await?;
        Ok(row.id)
    }

    // -----------------------------------------------------------------------
    // Single-value API
    // -----------------------------------------------------------------------

    pub async fn plugin_kv_single_set(
        self: &Arc<Self>,
        plugin_id: &str,
        key: &str,
        value: &str,
    ) -> BResult<()> {
        let db = self.db();
        let key_id = self
            .resolve_key_id(plugin_id, key, PluginKvKind::Single)
            .await?;
        let now = now_ms();
        let existing = plugin_kv_single::Entity::find_by_id(key_id)
            .one(&db)
            .await?;
        match existing {
            Some(row) => {
                let mut am: plugin_kv_single::ActiveModel = row.into();
                am.value = ActiveValue::Set(value.to_string());
                am.updated_at = ActiveValue::Set(now);
                am.update(&db).await?;
            }
            None => {
                let am = plugin_kv_single::ActiveModel {
                    key_id: Set(key_id),
                    value: Set(value.to_string()),
                    updated_at: Set(now),
                };
                am.insert(&db).await?;
            }
        }
        Ok(())
    }

    pub async fn plugin_kv_single_set_multi(
        self: &Arc<Self>,
        plugin_id: &str,
        entries: Vec<PluginKvEntry>,
    ) -> BResult<()> {
        let db = self.db();
        for entry in &entries {
            let key_id = self
                .resolve_key_id(plugin_id, &entry.key, PluginKvKind::Single)
                .await?;
            let now = now_ms();
            let existing = plugin_kv_single::Entity::find_by_id(key_id)
                .one(&db)
                .await?;
            match existing {
                Some(row) => {
                    let mut am: plugin_kv_single::ActiveModel = row.into();
                    am.value = ActiveValue::Set(entry.value.clone());
                    am.updated_at = ActiveValue::Set(now);
                    am.update(&db).await?;
                }
                None => {
                    let am = plugin_kv_single::ActiveModel {
                        key_id: Set(key_id),
                        value: Set(entry.value.clone()),
                        updated_at: Set(now),
                    };
                    am.insert(&db).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn plugin_kv_single_get(
        self: &Arc<Self>,
        plugin_id: &str,
        key: &str,
    ) -> BResult<Option<String>> {
        let db = self.db();
        let row = plugin_kv_single::Entity::find()
            .inner_join(plugin_kv_key::Entity)
            .filter(plugin_kv_key::Column::PluginId.eq(plugin_id))
            .filter(plugin_kv_key::Column::Key.eq(key))
            .filter(plugin_kv_key::Column::Kind.eq(PluginKvKind::Single.as_i32()))
            .one(&db)
            .await?;
        Ok(row.map(|r| r.value))
    }

    pub async fn plugin_kv_single_get_multi(
        self: &Arc<Self>,
        plugin_id: &str,
        keys: Vec<String>,
    ) -> BResult<Vec<PluginKvEntry>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let db = self.db();
        // Resolve key strings → ids for the IN clause, then fetch values,
        // then map ids back to strings. Two queries, both indexed.
        let key_rows = plugin_kv_key::Entity::find()
            .filter(plugin_kv_key::Column::PluginId.eq(plugin_id))
            .filter(plugin_kv_key::Column::Kind.eq(PluginKvKind::Single.as_i32()))
            .filter(plugin_kv_key::Column::Key.is_in(keys.clone()))
            .all(&db)
            .await?;
        if key_rows.is_empty() {
            return Ok(Vec::new());
        }
        let mut key_id_to_string: std::collections::HashMap<i64, String> =
            std::collections::HashMap::new();
        for kr in &key_rows {
            key_id_to_string.insert(kr.id, kr.key.clone());
        }
        let key_ids: Vec<i64> = key_rows.iter().map(|r| r.id).collect();
        let rows = plugin_kv_single::Entity::find()
            .filter(plugin_kv_single::Column::KeyId.is_in(key_ids))
            .all(&db)
            .await?;
        let mut by_key: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for r in rows {
            if let Some(s) = key_id_to_string.get(&r.key_id) {
                by_key.insert(s.clone(), r.value);
            }
        }
        // Preserve input key order; drop missing keys.
        let out: Vec<PluginKvEntry> = keys
            .into_iter()
            .filter_map(|k| {
                by_key
                    .remove(&k)
                    .map(|value| PluginKvEntry { key: k, value })
            })
            .collect();
        Ok(out)
    }

    pub async fn plugin_kv_single_delete(
        self: &Arc<Self>,
        plugin_id: &str,
        key: &str,
    ) -> BResult<()> {
        self.plugin_kv_single_delete_multi(plugin_id, vec![key.to_string()])
            .await
    }

    pub async fn plugin_kv_single_delete_multi(
        self: &Arc<Self>,
        plugin_id: &str,
        keys: Vec<String>,
    ) -> BResult<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let db = self.db();
        // Cascade via registry deletion (FK ON DELETE CASCADE removes the
        // single rows). Filter by plugin + kind to avoid touching another
        // plugin's keys if they happen to share a key string.
        plugin_kv_key::Entity::delete_many()
            .filter(plugin_kv_key::Column::PluginId.eq(plugin_id))
            .filter(plugin_kv_key::Column::Kind.eq(PluginKvKind::Single.as_i32()))
            .filter(plugin_kv_key::Column::Key.is_in(keys))
            .exec(&db)
            .await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Multi-value (append-only) API
    // -----------------------------------------------------------------------

    pub async fn plugin_kv_multi_append(
        self: &Arc<Self>,
        plugin_id: &str,
        key: &str,
        value: &str,
    ) -> BResult<()> {
        let db = self.db();
        let key_id = self
            .resolve_key_id(plugin_id, key, PluginKvKind::Multi)
            .await?;
        let now = now_ms();
        let am = plugin_kv_multi::ActiveModel {
            key_id: Set(key_id),
            value: Set(value.to_string()),
            created_at: Set(now),
            ..Default::default()
        };
        am.insert(&db).await?;
        Ok(())
    }

    pub async fn plugin_kv_multi_append_multi(
        self: &Arc<Self>,
        plugin_id: &str,
        entries: Vec<PluginKvEntry>,
    ) -> BResult<()> {
        let db = self.db();
        for entry in &entries {
            let key_id = self
                .resolve_key_id(plugin_id, &entry.key, PluginKvKind::Multi)
                .await?;
            let now = now_ms();
            let am = plugin_kv_multi::ActiveModel {
                key_id: Set(key_id),
                value: Set(entry.value.clone()),
                created_at: Set(now),
                ..Default::default()
            };
            am.insert(&db).await?;
        }
        Ok(())
    }

    pub async fn plugin_kv_multi_get_all(
        self: &Arc<Self>,
        plugin_id: &str,
        key: &str,
    ) -> BResult<Vec<String>> {
        let db = self.db();
        let key_row = plugin_kv_key::Entity::find()
            .filter(plugin_kv_key::Column::PluginId.eq(plugin_id))
            .filter(plugin_kv_key::Column::Key.eq(key))
            .filter(plugin_kv_key::Column::Kind.eq(PluginKvKind::Multi.as_i32()))
            .one(&db)
            .await?;
        let Some(key_row) = key_row else {
            return Ok(Vec::new());
        };
        let rows = plugin_kv_multi::Entity::find()
            .filter(plugin_kv_multi::Column::KeyId.eq(key_row.id))
            .order_by(plugin_kv_multi::Column::Id, Order::Asc)
            .all(&db)
            .await?;
        Ok(rows.into_iter().map(|r| r.value).collect())
    }

    /// Returns `PluginKvMultiEntry` per requested key (omits keys with no
    /// values). One indexed `IN (...)` scan.
    pub async fn plugin_kv_multi_get_all_multi(
        self: &Arc<Self>,
        plugin_id: &str,
        keys: Vec<String>,
    ) -> BResult<Vec<PluginKvMultiEntry>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let db = self.db();
        let key_rows = plugin_kv_key::Entity::find()
            .filter(plugin_kv_key::Column::PluginId.eq(plugin_id))
            .filter(plugin_kv_key::Column::Kind.eq(PluginKvKind::Multi.as_i32()))
            .filter(plugin_kv_key::Column::Key.is_in(keys.clone()))
            .all(&db)
            .await?;
        if key_rows.is_empty() {
            return Ok(Vec::new());
        }
        let mut key_id_to_string: std::collections::HashMap<i64, String> =
            std::collections::HashMap::new();
        for kr in &key_rows {
            key_id_to_string.insert(kr.id, kr.key.clone());
        }
        let key_ids: Vec<i64> = key_rows.iter().map(|r| r.id).collect();
        let rows = plugin_kv_multi::Entity::find()
            .filter(plugin_kv_multi::Column::KeyId.is_in(key_ids))
            .order_by(plugin_kv_multi::Column::KeyId, Order::Asc)
            .order_by(plugin_kv_multi::Column::Id, Order::Asc)
            .all(&db)
            .await?;

        let mut grouped: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for r in rows {
            if let Some(s) = key_id_to_string.get(&r.key_id) {
                grouped.entry(s.clone()).or_default().push(r.value);
            }
        }
        // Preserve input key order; omit keys with no values.
        let out: Vec<PluginKvMultiEntry> = keys
            .into_iter()
            .filter_map(|k| {
                grouped
                    .remove(&k)
                    .map(|values| PluginKvMultiEntry { key: k, values })
            })
            .collect();
        Ok(out)
    }

    pub async fn plugin_kv_multi_count(
        self: &Arc<Self>,
        plugin_id: &str,
        key: &str,
    ) -> BResult<u64> {
        let db = self.db();
        let key_row = plugin_kv_key::Entity::find()
            .filter(plugin_kv_key::Column::PluginId.eq(plugin_id))
            .filter(plugin_kv_key::Column::Key.eq(key))
            .filter(plugin_kv_key::Column::Kind.eq(PluginKvKind::Multi.as_i32()))
            .one(&db)
            .await?;
        let Some(key_row) = key_row else {
            return Ok(0);
        };
        let count = plugin_kv_multi::Entity::find()
            .filter(plugin_kv_multi::Column::KeyId.eq(key_row.id))
            .count(&db)
            .await?;
        Ok(count)
    }

    /// The play-count hot path: one SQL `COUNT(*) GROUP BY key_id` over
    /// the requested key set. Returns one `PluginKvCountEntry` per key
    /// that has at least one value (keys with zero values are omitted).
    pub async fn plugin_kv_multi_count_multi(
        self: &Arc<Self>,
        plugin_id: &str,
        keys: Vec<String>,
    ) -> BResult<Vec<PluginKvCountEntry>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let db = self.db();
        let key_rows = plugin_kv_key::Entity::find()
            .filter(plugin_kv_key::Column::PluginId.eq(plugin_id))
            .filter(plugin_kv_key::Column::Kind.eq(PluginKvKind::Multi.as_i32()))
            .filter(plugin_kv_key::Column::Key.is_in(keys.clone()))
            .all(&db)
            .await?;
        if key_rows.is_empty() {
            return Ok(Vec::new());
        }
        let mut key_id_to_string: std::collections::HashMap<i64, String> =
            std::collections::HashMap::new();
        for kr in &key_rows {
            key_id_to_string.insert(kr.id, kr.key.clone());
        }
        let key_ids_csv: String = key_rows
            .iter()
            .map(|r| r.id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        // Parameterized SQL via Statement::from_string — key_ids are i64
        // values we just read from the DB, so the CSV is safe from
        // injection. Parameter binding for variable-length IN lists isn't
        // supported by sqlx-sqlite without dynamic placeholder generation,
        // so we inline the validated integers.
        let sql = format!(
            "SELECT key_id, COUNT(*) AS n FROM plugin_kv_multi \
             WHERE key_id IN ({}) GROUP BY key_id",
            key_ids_csv
        );
        #[derive(FromQueryResult)]
        struct RowCount {
            key_id: i64,
            n: i64,
        }
        let counts = RowCount::find_by_statement(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            sql,
            vec![],
        ))
        .all(&db)
        .await?;

        let mut by_key: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        for c in counts {
            if let Some(s) = key_id_to_string.get(&c.key_id) {
                by_key.insert(s.clone(), c.n as u64);
            }
        }
        // Preserve input key order; keys with zero values are dropped.
        let out: Vec<PluginKvCountEntry> = keys
            .into_iter()
            .filter_map(|k| {
                by_key
                    .remove(&k)
                    .map(|count| PluginKvCountEntry { key: k, count })
            })
            .collect();
        Ok(out)
    }

    pub async fn plugin_kv_multi_delete(
        self: &Arc<Self>,
        plugin_id: &str,
        key: &str,
    ) -> BResult<()> {
        self.plugin_kv_multi_delete_multi(plugin_id, vec![key.to_string()])
            .await
    }

    pub async fn plugin_kv_multi_delete_multi(
        self: &Arc<Self>,
        plugin_id: &str,
        keys: Vec<String>,
    ) -> BResult<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let db = self.db();
        plugin_kv_key::Entity::delete_many()
            .filter(plugin_kv_key::Column::PluginId.eq(plugin_id))
            .filter(plugin_kv_key::Column::Kind.eq(PluginKvKind::Multi.as_i32()))
            .filter(plugin_kv_key::Column::Key.is_in(keys))
            .exec(&db)
            .await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Key listing
    // -----------------------------------------------------------------------

    /// Lists all registered keys for `plugin_id` whose key string starts
    /// with `prefix`. Returns `{key, kind}` for each, sorted by key asc.
    pub async fn plugin_kv_list_keys(
        self: &Arc<Self>,
        plugin_id: &str,
        prefix: &str,
    ) -> BResult<Vec<PluginKvKeyInfo>> {
        let db = self.db();
        let mut q =
            plugin_kv_key::Entity::find().filter(plugin_kv_key::Column::PluginId.eq(plugin_id));
        if !prefix.is_empty() {
            // LIKE 'prefix%' — escape any embedded `%`/`_` in the prefix.
            let escaped: String = prefix
                .chars()
                .flat_map(|c| match c {
                    '%' | '_' | '\\' => vec!['\\', c],
                    _ => vec![c],
                })
                .collect();
            q = q.filter(plugin_kv_key::Column::Key.like(format!("{}%", escaped)));
        }
        let rows = q
            .order_by(plugin_kv_key::Column::Key, Order::Asc)
            .all(&db)
            .await?;
        let out: Vec<PluginKvKeyInfo> = rows
            .into_iter()
            .filter_map(|r| {
                PluginKvKind::from_i32(r.kind).map(|kind| PluginKvKeyInfo { key: r.key, kind })
            })
            .collect();
        Ok(out)
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
