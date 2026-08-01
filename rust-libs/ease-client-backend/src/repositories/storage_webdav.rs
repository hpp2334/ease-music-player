//! WebDAV storage detail CRUD + the create/update flow.
//!
//! Split from `storage.rs` (the kind-agnostic registry): WebDAV rows live in
//! the `webdav_storage` table, with the password stored separately in the
//! `secret` table (scope = `"internal"`). `upsert_webdav_storage` orchestrates
//! secret rotation through the [`SecretStore`] trait.

use std::sync::Arc;

use ease_client_schema::entities::webdav_storage;
use ease_client_schema::{
    SecretId, SecretScope, StorageHandle, StorageId, WebdavStorageId,
};
use sea_orm::{ActiveModelTrait, ActiveValue, ActiveValue::Set, EntityTrait};

use crate::{
    error::{BError, BResult},
    objects::ArgUpsertWebdavStorage,
};

use super::core::DatabaseServer;
use super::secret::SecretStore;

impl DatabaseServer {
    pub async fn load_webdav_storage(
        &self,
        id: WebdavStorageId,
    ) -> BResult<Option<webdav_storage::Model>> {
        Ok(webdav_storage::Entity::find_by_id(*id.as_ref())
            .one(&self.db())
            .await?)
    }

    /// Drop a `webdav_storage` detail row + its internal-scoped secret.
    /// Called by `remove_storage` when cascading a WebDAV registry row.
    pub async fn remove_webdav_storage_detail(self: &Arc<Self>, wid: i64) -> BResult<()> {
        let db = self.db();
        if let Some(w) = webdav_storage::Entity::find_by_id(wid).one(&db).await? {
            if let Some(sid) = w.secret_id {
                self.secret_remove(SecretScope::Internal, SecretId::wrap(sid))
                    .await?;
            }
            webdav_storage::Entity::delete_by_id(wid).exec(&db).await?;
        }
        Ok(())
    }

    /// Create or update a WebDAV storage. Returns the registry `StorageId`.
    ///
    /// - **Create** (`arg.id` = None): `secret_put` -> id, insert
    ///   `webdav_storage`, `obtain(Webdav)` -> registry id.
    /// - **Update** (`arg.id` = Some): load the registry row -> its
    ///   `webdav_storage_id` -> update the row; rotate the secret when
    ///   `password` is non-empty (blank on edit = keep the existing secret).
    pub async fn upsert_webdav_storage(
        self: &Arc<Self>,
        arg: ArgUpsertWebdavStorage,
    ) -> BResult<StorageId> {
        let db = self.db();
        match arg.id {
            Some(registry_id) => {
                let reg = self
                    .load_storage_row(registry_id)
                    .await?
                    .ok_or_else(|| BError::CustomError {
                        message: "storage not found".into(),
                    })?;
                let wid = reg.webdav_storage_id.ok_or_else(|| BError::CustomError {
                    message: "storage is not a WebDAV storage".into(),
                })?;
                let existing = webdav_storage::Entity::find_by_id(wid)
                    .one(&db)
                    .await?
                    .ok_or_else(|| BError::CustomError {
                        message: "webdav_storage row missing".into(),
                    })?;

                let secret_id = if arg.password.is_empty() {
                    existing.secret_id // keep
                } else {
                    let new_sid =
                        self.secret_put(SecretScope::Internal, arg.password.clone())
                            .await?;
                    if let Some(old) = existing.secret_id {
                        self.secret_remove(SecretScope::Internal, SecretId::wrap(old))
                            .await?;
                    }
                    Some(*new_sid.as_ref())
                };

                let mut am: webdav_storage::ActiveModel = existing.into();
                am.addr = Set(arg.addr);
                am.alias = Set(arg.alias);
                am.username = Set(arg.username);
                am.secret_id = Set(secret_id);
                am.is_anonymous = Set(if arg.is_anonymous { 1 } else { 0 });
                am.update(&db).await?;
                Ok(registry_id)
            }
            None => {
                let secret_id = if arg.password.is_empty() {
                    None
                } else {
                    let sid =
                        self.secret_put(SecretScope::Internal, arg.password.clone())
                            .await?;
                    Some(*sid.as_ref())
                };
                let am = webdav_storage::ActiveModel {
                    id: ActiveValue::NotSet,
                    addr: Set(arg.addr),
                    alias: Set(arg.alias),
                    username: Set(arg.username),
                    secret_id: Set(secret_id),
                    is_anonymous: Set(if arg.is_anonymous { 1 } else { 0 }),
                };
                let w = am.insert(&db).await?;
                self.obtain_storage(&StorageHandle::Webdav {
                    webdav_storage_id: WebdavStorageId::wrap(w.id),
                })
                .await
            }
        }
    }
}
