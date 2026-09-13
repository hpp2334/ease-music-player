//! Owner-scoped secret store.
//!
//! Backs the `secret` table on Android (values plaintext in SQLite — same
//! exposure as the pre-refactor `storage.password` column). The trait abstraction
//! lets a future non-Android build back it with the platform keychain instead.
//!
//! Ownership is enforced at this layer: `get` / `remove` only succeed when the
//! row's `scope` matches the caller's [`SecretScope`]. The plugin-facing
//! capability (Stage 4) additionally binds the scope to the calling plugin, so
//! a plugin cannot even express another principal's scope.

use std::sync::Arc;

use ease_client_schema::entities::secret;
use ease_client_schema::{PluginId, SecretId, SecretScope};
use futures::future::BoxFuture;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use crate::error::BResult;

use super::core::DatabaseServer;

/// Owner-scoped secret access. All methods are scoped: a row whose `scope`
/// does not match the caller's is invisible (`get` -> `None`, `remove` ->
/// no-op).
pub trait SecretStore: Send + Sync {
    fn secret_put(&self, scope: SecretScope, secret: String) -> BoxFuture<'_, BResult<SecretId>>;
    fn secret_get(
        &self,
        scope: SecretScope,
        id: SecretId,
    ) -> BoxFuture<'_, BResult<Option<String>>>;
    fn secret_remove(&self, scope: SecretScope, id: SecretId) -> BoxFuture<'_, BResult<()>>;
}

impl SecretStore for DatabaseServer {
    fn secret_put(&self, scope: SecretScope, secret: String) -> BoxFuture<'_, BResult<SecretId>> {
        Box::pin(async move {
            let db = self.db();
            let am = secret::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                scope: Set(scope.to_scope_string()),
                secret: Set(secret),
            };
            let m = am.insert(&db).await?;
            Ok(SecretId::wrap(m.id))
        })
    }

    fn secret_get(
        &self,
        scope: SecretScope,
        id: SecretId,
    ) -> BoxFuture<'_, BResult<Option<String>>> {
        Box::pin(async move {
            let db = self.db();
            let row = secret::Entity::find_by_id(*id.as_ref()).one(&db).await?;
            match row {
                Some(r) if r.scope == scope.to_scope_string() => Ok(Some(r.secret)),
                _ => Ok(None),
            }
        })
    }

    fn secret_remove(&self, scope: SecretScope, id: SecretId) -> BoxFuture<'_, BResult<()>> {
        Box::pin(async move {
            let db = self.db();
            let row = secret::Entity::find_by_id(*id.as_ref()).one(&db).await?;
            if matches!(row, Some(r) if r.scope == scope.to_scope_string()) {
                secret::Entity::delete_by_id(*id.as_ref()).exec(&db).await?;
            }
            Ok(())
        })
    }
}

impl DatabaseServer {
    /// Host-level bulk wipe of every secret owned by `plugin_id`
    /// (uninstall). The scope-string match leaves `internal` and other
    /// plugins' rows untouched. Deliberately not on the [`SecretStore`]
    /// trait — that stays owner-scoped, single-id access.
    pub async fn secret_remove_all_for_plugin(
        self: &Arc<Self>,
        plugin_id: &str,
    ) -> BResult<()> {
        let db = self.db();
        let scope = SecretScope::Plugin(PluginId::new(plugin_id.to_string())).to_scope_string();
        secret::Entity::delete_many()
            .filter(secret::Column::Scope.eq(scope))
            .exec(&db)
            .await?;
        Ok(())
    }
}
