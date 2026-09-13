use std::sync::Arc;

use ease_client_migration::converter;
use ease_client_schema::entities::{playlist, playlist_group};
use ease_client_schema::{PlaylistGroupId, PlaylistGroupModel, PlaylistId};
use ease_order_key::{OrderKey, OrderKeyRef};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};

use crate::error::{BError, BResult};

use super::core::DatabaseServer;

impl DatabaseServer {
    pub async fn load_playlist_groups(self: &Arc<Self>) -> BResult<Vec<PlaylistGroupModel>> {
        let db = self.db();
        let rows = playlist_group::Entity::find().all(&db).await?;
        let mut ret: Vec<PlaylistGroupModel> = rows
            .into_iter()
            .map(converter::playlist_group_to_model)
            .collect();
        ret.sort_by_key(|v| OrderKey::wrap(v.order.clone()));
        Ok(ret)
    }

    pub async fn load_playlist_group(
        self: &Arc<Self>,
        id: PlaylistGroupId,
    ) -> BResult<Option<PlaylistGroupModel>> {
        let db = self.db();
        let row = playlist_group::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        Ok(row.map(converter::playlist_group_to_model))
    }

    pub async fn create_playlist_group(
        self: &Arc<Self>,
        title: String,
        current_time_ms: i64,
        order: OrderKey,
    ) -> BResult<PlaylistGroupId> {
        let db = self.db();
        let am = playlist_group::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(title),
            created_time: ActiveValue::Set(current_time_ms),
            order: ActiveValue::Set(serde_json::to_string(&order.into_raw())?),
            // New groups start expanded.
            expanded: ActiveValue::Set(1),
        };
        let inserted = am.insert(&db).await?;
        Ok(PlaylistGroupId::wrap(inserted.id))
    }

    pub async fn update_playlist_group(
        self: &Arc<Self>,
        id: PlaylistGroupId,
        title: String,
    ) -> BResult<PlaylistGroupId> {
        let db = self.db();
        let row = playlist_group::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: playlist_group::ActiveModel = row.into();
            am.title = ActiveValue::Set(title);
            am.update(&db).await?;
        }
        Ok(id)
    }

    pub async fn set_playlist_group_order(
        self: &Arc<Self>,
        id: PlaylistGroupId,
        order: OrderKey,
    ) -> BResult<()> {
        let db = self.db();
        let row = playlist_group::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: playlist_group::ActiveModel = row.into();
            am.order = ActiveValue::Set(serde_json::to_string(&order.into_raw())?);
            am.update(&db).await?;
        }
        Ok(())
    }

    /// Persist the UI expand/collapse state for one group. Preference
    /// write only — never validated beyond the row update.
    pub async fn set_playlist_group_expanded(
        self: &Arc<Self>,
        id: PlaylistGroupId,
        expanded: bool,
    ) -> BResult<()> {
        let db = self.db();
        let row = playlist_group::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: playlist_group::ActiveModel = row.into();
            am.expanded = ActiveValue::Set(if expanded { 1 } else { 0 });
            am.update(&db).await?;
        }
        Ok(())
    }

    /// Delete a group. With `delete_playlists = false` its playlists
    /// are swept into the first remaining group (order-key order);
    /// with `true` every member playlist is removed through the normal
    /// playlist-removal cascade (edges → music compaction → cover
    /// blobs). The last group can never be deleted either way —
    /// playlists must always live in a group.
    pub async fn remove_playlist_group(
        self: &Arc<Self>,
        id: PlaylistGroupId,
        delete_playlists: bool,
    ) -> BResult<()> {
        let db = self.db();

        let mut groups = self.load_playlist_groups().await?;
        if !groups.iter().any(|g| g.id == id) {
            // Already gone — idempotent delete.
            return Ok(());
        }
        if groups.len() <= 1 {
            return Err(BError::LastGroupCannotRemove);
        }

        if delete_playlists {
            let members = playlist::Entity::find()
                .filter(playlist::Column::GroupId.eq(*id.as_ref()))
                .all(&db)
                .await?;
            for m in members {
                self.remove_playlist(PlaylistId::wrap(m.id)).await?;
            }
        } else {
            groups.retain(|g| g.id != id);
            let sweep_target = groups[0].id;

            playlist::Entity::update_many()
                .col_expr(
                    playlist::Column::GroupId,
                    sea_orm::sea_query::Expr::value(*sweep_target.as_ref()),
                )
                .filter(playlist::Column::GroupId.eq(*id.as_ref()))
                .exec(&db)
                .await?;
        }

        playlist_group::Entity::delete_by_id(*id.as_ref()).exec(&db).await?;
        Ok(())
    }

    /// The self-heal behind "always create a Default group": if no group
    /// exists, create one (`default_title`, falling back to "Default",
    /// expanded); then sweep every orphaned playlist (NULL or dangling
    /// `group_id`) into the first group. Idempotent — a no-op on a
    /// healthy database. Returns the first group's id.
    pub async fn ensure_playlist_groups(
        self: &Arc<Self>,
        default_title: Option<String>,
        current_time_ms: i64,
    ) -> BResult<PlaylistGroupId> {
        let db = self.db();

        let mut groups = self.load_playlist_groups().await?;
        if groups.is_empty() {
            let title = default_title.unwrap_or_else(|| "Default".to_string());
            self.create_playlist_group(
                title,
                current_time_ms,
                OrderKey::greater(&OrderKey::default()),
            )
            .await?;
            groups = self.load_playlist_groups().await?;
        }
        let first = groups
            .first()
            .expect("ensure created a group")
            .clone();

        let target = *first.id.as_ref();

        // Orphans with no group at all (pre-group databases).
        playlist::Entity::update_many()
            .col_expr(playlist::Column::GroupId, sea_orm::sea_query::Expr::value(target))
            .filter(playlist::Column::GroupId.is_null())
            .exec(&db)
            .await?;

        // Dangling references (group rows that no longer exist).
        let valid: Vec<i64> = groups.iter().map(|g| *g.id.as_ref()).collect();
        let rows = playlist::Entity::find().all(&db).await?;
        let mut dangling: Vec<i64> = rows
            .iter()
            .filter_map(|r| r.group_id)
            .filter(|gid| !valid.contains(gid))
            .collect();
        dangling.sort_unstable();
        dangling.dedup();
        for gid in dangling {
            playlist::Entity::update_many()
                .col_expr(playlist::Column::GroupId, sea_orm::sea_query::Expr::value(target))
                .filter(playlist::Column::GroupId.eq(gid))
                .exec(&db)
                .await?;
        }

        Ok(first.id)
    }

    /// Move a playlist into another group at a specific position: sets
    /// `group_id` and derives the new order key from the destination
    /// neighbors `a` (previous) / `b` (next) — both `None` (empty
    /// destination) falls back to [`OrderKey::default`]. Same-group
    /// calls degrade to a plain reorder.
    pub async fn move_playlist_to_group_at(
        self: &Arc<Self>,
        playlist_id: PlaylistId,
        group_id: PlaylistGroupId,
        a: Option<PlaylistId>,
        b: Option<PlaylistId>,
    ) -> BResult<()> {
        let db = self.db();

        let group = playlist_group::Entity::find_by_id(*group_id.as_ref())
            .one(&db)
            .await?
            .ok_or(BError::GroupNotFound(group_id))?;
        let row = playlist::Entity::find_by_id(*playlist_id.as_ref())
            .one(&db)
            .await?
            .ok_or(BError::PlaylistNotFound(playlist_id))?;

        async fn load_order(
            db: &sea_orm::DatabaseConnection,
            id: PlaylistId,
        ) -> BResult<Option<OrderKey>> {
            Ok(playlist::Entity::find_by_id(*id.as_ref())
                .one(db)
                .await?
                .map(|r| OrderKey::wrap(converter::decode_order(&r.order))))
        }

        let a_order = match a {
            Some(id) => load_order(&db, id).await?,
            None => None,
        };
        let b_order = match b {
            Some(id) => load_order(&db, id).await?,
            None => None,
        };
        let order = match (a_order, b_order) {
            (Some(a_order), Some(b_order)) => OrderKey::between(
                OrderKeyRef::wrap(&a_order.into_raw()),
                OrderKeyRef::wrap(&b_order.into_raw()),
            )?,
            (Some(a_order), None) => OrderKey::greater(&a_order),
            (None, Some(b_order)) => OrderKey::less_or_fallback(&b_order),
            (None, None) => OrderKey::default(),
        };

        let mut am: playlist::ActiveModel = row.into();
        am.group_id = ActiveValue::Set(Some(group.id));
        am.order = ActiveValue::Set(serde_json::to_string(&order.into_raw())?);
        am.update(&db).await?;
        Ok(())
    }

    /// Move a playlist into another group, re-keying it after the
    /// destination group's current last playlist so it lands at the end
    /// of its new section. No-op when already a member.
    pub async fn move_playlist_to_group(
        self: &Arc<Self>,
        playlist_id: PlaylistId,
        group_id: PlaylistGroupId,
    ) -> BResult<()> {
        let db = self.db();

        let group = playlist_group::Entity::find_by_id(*group_id.as_ref())
            .one(&db)
            .await?
            .ok_or(BError::GroupNotFound(group_id))?;
        let row = playlist::Entity::find_by_id(*playlist_id.as_ref())
            .one(&db)
            .await?
            .ok_or(BError::PlaylistNotFound(playlist_id))?;

        if row.group_id == Some(group.id) {
            return Ok(());
        }

        let group_rows = playlist::Entity::find()
            .filter(playlist::Column::GroupId.eq(group.id))
            .all(&db)
            .await?;
        let last_order = group_rows
            .iter()
            .map(|r| OrderKey::wrap(converter::decode_order(&r.order)))
            .max()
            .unwrap_or_default();

        let mut am: playlist::ActiveModel = row.into();
        am.group_id = ActiveValue::Set(Some(group.id));
        am.order =
            ActiveValue::Set(serde_json::to_string(&OrderKey::greater(&last_order).into_raw())?);
        am.update(&db).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::music::ArgDBAddMusic;
    use crate::repositories::playlist::ArgDBCreatePlaylist;
    use ease_client_schema::{PlaylistId, StorageEntryLoc, StorageId};

    async fn server() -> (Arc<DatabaseServer>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let server = DatabaseServer::new();
        server
            .init(dir.path().to_str().unwrap().to_string())
            .await
            .unwrap();
        (server, dir)
    }

    fn add_music_arg(path: &str) -> ArgDBAddMusic {
        ArgDBAddMusic {
            loc: StorageEntryLoc {
                storage_id: StorageId::wrap(1),
                path: path.to_string(),
            },
            title: path.to_string(),
        }
    }

    async fn insert_raw_playlist(server: &DatabaseServer, order: &str, group: Option<i64>) -> i64 {
        let am = playlist::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set("raw".to_string()),
            created_time: ActiveValue::Set(0),
            picture_storage_id: ActiveValue::Set(None),
            picture_path: ActiveValue::Set(None),
            order: ActiveValue::Set(order.to_string()),
            storage_allowlist: ActiveValue::Set(None),
            group_id: ActiveValue::Set(group),
        };
        am.insert(&server.db()).await.unwrap().id
    }

    #[tokio::test]
    async fn ensure_creates_localized_default_group_expanded() {
        let (server, _dir) = server().await;

        let first = server
            .ensure_playlist_groups(Some("默认".to_string()), 42)
            .await
            .unwrap();

        let groups = server.load_playlist_groups().await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, first);
        assert_eq!(groups[0].title, "默认");
        assert!(groups[0].expanded);
        assert_eq!(groups[0].created_time, 42);
    }

    #[tokio::test]
    async fn ensure_falls_back_to_english_default_title() {
        let (server, _dir) = server().await;

        server.ensure_playlist_groups(None, 0).await.unwrap();

        let groups = server.load_playlist_groups().await.unwrap();
        assert_eq!(groups[0].title, "Default");
    }

    #[tokio::test]
    async fn ensure_sweeps_null_and_dangling_orphans_into_first_group() {
        let (server, _dir) = server().await;

        // Pre-group rows: one with NULL group, one pointing at a group
        // row that does not exist.
        let null_orphan = insert_raw_playlist(&server, "[1]", None).await;
        let dangling_orphan = insert_raw_playlist(&server, "[2]", Some(999)).await;

        let first = server.ensure_playlist_groups(None, 0).await.unwrap();

        let playlists = server.load_playlists().await.unwrap();
        let by_id = |id: i64| {
            playlists
                .iter()
                .find(|p| *p.id.as_ref() == id)
                .unwrap()
                .clone()
        };
        assert_eq!(by_id(null_orphan).group_id, Some(first));
        assert_eq!(by_id(dangling_orphan).group_id, Some(first));
    }

    #[tokio::test]
    async fn ensure_is_idempotent() {
        let (server, _dir) = server().await;

        server.ensure_playlist_groups(None, 0).await.unwrap();
        server.ensure_playlist_groups(None, 0).await.unwrap();

        assert_eq!(server.load_playlist_groups().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn create_playlist_persists_group() {
        let (server, _dir) = server().await;

        let g1 = server
            .create_playlist_group("G1".into(), 0, OrderKey::default())
            .await
            .unwrap();
        let g2 = server
            .create_playlist_group("G2".into(), 0, OrderKey::greater(&OrderKey::default()))
            .await
            .unwrap();

        // The repo persists the given order + group verbatim; the
        // group-slice order computation lives in `ct_create_playlist`.
        let (p1, _) = server
            .create_playlist(ArgDBCreatePlaylist {
                title: "p1".into(),
                picture: None,
                musics: vec![],
                current_time_ms: 0,
                order: OrderKey::wrap(vec![1]),
                storage_allowlist: None,
                group_id: g2,
            })
            .await
            .unwrap();
        let (_, _) = server
            .create_playlist(ArgDBCreatePlaylist {
                title: "in-g1".into(),
                picture: None,
                musics: vec![add_music_arg("/a.mp3")],
                current_time_ms: 0,
                order: OrderKey::wrap(vec![2]),
                storage_allowlist: None,
                group_id: g1,
            })
            .await
            .unwrap();
        let (p3, _) = server
            .create_playlist(ArgDBCreatePlaylist {
                title: "p3".into(),
                picture: None,
                musics: vec![],
                current_time_ms: 0,
                order: OrderKey::greater(&OrderKey::wrap(vec![1])),
                storage_allowlist: None,
                group_id: g2,
            })
            .await
            .unwrap();

        let playlists = server.load_playlists().await.unwrap();
        let by_id = |id: PlaylistId| {
            playlists
                .iter()
                .find(|p| p.id == id)
                .unwrap()
                .clone()
        };
        assert_eq!(by_id(p1).group_id, Some(g2));
        assert_eq!(by_id(p3).group_id, Some(g2));
        // Ascending keys within the group slice.
        assert!(by_id(p3).order > by_id(p1).order);
    }

    #[tokio::test]
    async fn move_playlist_to_group_rekeys_to_end_of_destination() {
        let (server, _dir) = server().await;

        let g1 = server
            .create_playlist_group("G1".into(), 0, OrderKey::greater(&OrderKey::default()))
            .await
            .unwrap();
        let g2 = server
            .create_playlist_group("G2".into(), 0, OrderKey::greater(&OrderKey::wrap(vec![1])))
            .await
            .unwrap();

        let (mover, _) = server
            .create_playlist(ArgDBCreatePlaylist {
                title: "mover".into(),
                picture: None,
                musics: vec![],
                current_time_ms: 0,
                order: OrderKey::wrap(vec![1]),
                storage_allowlist: None,
                group_id: g1,
            })
            .await
            .unwrap();
        let (anchor, _) = server
            .create_playlist(ArgDBCreatePlaylist {
                title: "anchor".into(),
                picture: None,
                musics: vec![],
                current_time_ms: 0,
                order: OrderKey::wrap(vec![5]),
                storage_allowlist: None,
                group_id: g2,
            })
            .await
            .unwrap();

        // Same-group move is a no-op.
        server.move_playlist_to_group(anchor, g2).await.unwrap();

        server.move_playlist_to_group(mover, g2).await.unwrap();

        let playlists = server.load_playlists().await.unwrap();
        let moved = playlists.iter().find(|p| p.id == mover).unwrap();
        let anchor_row = playlists.iter().find(|p| p.id == anchor).unwrap();
        assert_eq!(moved.group_id, Some(g2));
        // Lands strictly after the destination's current last playlist.
        assert!(moved.order > anchor_row.order);
    }

    #[tokio::test]
    async fn move_to_missing_group_or_playlist_errors() {
        let (server, _dir) = server().await;
        let g = server.ensure_playlist_groups(None, 0).await.unwrap();

        let missing_group = PlaylistGroupId::wrap(999);
        assert!(matches!(
            server.move_playlist_to_group(PlaylistId::wrap(1), missing_group).await,
            Err(BError::GroupNotFound(_))
        ));
        // Valid group, missing playlist.
        assert!(matches!(
            server.move_playlist_to_group(PlaylistId::wrap(1), g).await,
            Err(BError::PlaylistNotFound(_))
        ));
    }

    #[tokio::test]
    async fn remove_group_sweeps_playlists_into_first_remaining_group() {
        let (server, _dir) = server().await;

        let g1 = server
            .create_playlist_group("G1".into(), 0, OrderKey::greater(&OrderKey::default()))
            .await
            .unwrap();
        let g2 = server
            .create_playlist_group("G2".into(), 0, OrderKey::greater(&OrderKey::wrap(vec![1])))
            .await
            .unwrap();

        let (in_g2, _) = server
            .create_playlist(ArgDBCreatePlaylist {
                title: "in-g2".into(),
                picture: None,
                musics: vec![],
                current_time_ms: 0,
                order: OrderKey::greater(&OrderKey::default()),
                storage_allowlist: None,
                group_id: g2,
            })
            .await
            .unwrap();

        server.remove_playlist_group(g2, false).await.unwrap();

        let groups = server.load_playlist_groups().await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, g1);
        let playlists = server.load_playlists().await.unwrap();
        assert_eq!(
            playlists.iter().find(|p| p.id == in_g2).unwrap().group_id,
            Some(g1)
        );

        // The last group can never be removed.
        assert!(matches!(
            server.remove_playlist_group(g1, false).await,
            Err(BError::LastGroupCannotRemove)
        ));
        assert!(matches!(
            server.remove_playlist_group(g1, true).await,
            Err(BError::LastGroupCannotRemove)
        ));
    }

    #[tokio::test]
    async fn remove_group_cascades_playlists_when_requested() {
        let (server, _dir) = server().await;

        let g1 = server.ensure_playlist_groups(None, 0).await.unwrap();
        let g2 = server
            .create_playlist_group("G2".into(), 0, OrderKey::greater(&OrderKey::default()))
            .await
            .unwrap();

        // Two playlists in G2, one with a music edge so the cascade
        // exercises the edge + music-compaction path too.
        let (keep, _) = server
            .create_playlist(ArgDBCreatePlaylist {
                title: "keep".into(),
                picture: None,
                musics: vec![],
                current_time_ms: 0,
                order: OrderKey::default(),
                storage_allowlist: None,
                group_id: g1,
            })
            .await
            .unwrap();
        let (doomed, _) = server
            .create_playlist(ArgDBCreatePlaylist {
                title: "doomed".into(),
                picture: None,
                musics: vec![add_music_arg("/doomed.mp3")],
                current_time_ms: 0,
                order: OrderKey::default(),
                storage_allowlist: None,
                group_id: g2,
            })
            .await
            .unwrap();
        let (doomed_empty, _) = server
            .create_playlist(ArgDBCreatePlaylist {
                title: "doomed-empty".into(),
                picture: None,
                musics: vec![],
                current_time_ms: 0,
                order: OrderKey::default(),
                storage_allowlist: None,
                group_id: g2,
            })
            .await
            .unwrap();

        server.remove_playlist_group(g2, true).await.unwrap();

        let groups = server.load_playlist_groups().await.unwrap();
        assert_eq!(groups.iter().map(|g| g.id).collect::<Vec<_>>(), vec![g1]);
        let playlists = server.load_playlists().await.unwrap();
        let ids = playlists.iter().map(|p| p.id).collect::<Vec<_>>();
        assert!(ids.contains(&keep), "playlist outside the group survives");
        assert!(!ids.contains(&doomed), "group member removed");
        assert!(!ids.contains(&doomed_empty), "empty group member removed");
    }

    #[tokio::test]
    async fn move_playlist_to_group_at_lands_between_neighbors() {
        let (server, _dir) = server().await;

        let g1 = server.ensure_playlist_groups(None, 0).await.unwrap();
        let g2 = server
            .create_playlist_group("G2".into(), 0, OrderKey::greater(&OrderKey::default()))
            .await
            .unwrap();

        let mk = |title: &str, group, order: Vec<u32>| ArgDBCreatePlaylist {
            title: title.to_string(),
            picture: None,
            musics: vec![],
            current_time_ms: 0,
            order: OrderKey::wrap(order),
            storage_allowlist: None,
            group_id: group,
        };
        let (mover, _) = server.create_playlist(mk("mover", g1, vec![1])).await.unwrap();
        let (t1, _) = server.create_playlist(mk("t1", g2, vec![10])).await.unwrap();
        let (t2, _) = server.create_playlist(mk("t2", g2, vec![20])).await.unwrap();

        // Between t1 and t2.
        server
            .move_playlist_to_group_at(mover, g2, Some(t1), Some(t2))
            .await
            .unwrap();
        let playlists = server.load_playlists().await.unwrap();
        let get = |id| playlists.iter().find(|p| p.id == id).unwrap().clone();
        let moved = get(mover);
        assert_eq!(moved.group_id, Some(g2));
        assert!(moved.order > get(t1).order);
        assert!(moved.order < get(t2).order);

        // To the head (b only).
        server
            .move_playlist_to_group_at(mover, g2, None, Some(t1))
            .await
            .unwrap();
        let playlists = server.load_playlists().await.unwrap();
        let get = |id| playlists.iter().find(|p| p.id == id).unwrap().clone();
        assert!(get(mover).order < get(t1).order);

        // Into a fresh empty group (no neighbors).
        let g3 = server
            .create_playlist_group("G3".into(), 0, OrderKey::greater(&OrderKey::wrap(vec![20])))
            .await
            .unwrap();
        server
            .move_playlist_to_group_at(mover, g3, None, None)
            .await
            .unwrap();
        let playlists = server.load_playlists().await.unwrap();
        assert_eq!(
            playlists.iter().find(|p| p.id == mover).unwrap().group_id,
            Some(g3)
        );
    }

    #[tokio::test]
    async fn expanded_state_persists() {
        let (server, _dir) = server().await;
        let g = server.ensure_playlist_groups(None, 0).await.unwrap();

        server.set_playlist_group_expanded(g, false).await.unwrap();
        assert!(!server.load_playlist_groups().await.unwrap()[0].expanded);
        server.set_playlist_group_expanded(g, true).await.unwrap();
        assert!(server.load_playlist_groups().await.unwrap()[0].expanded);
    }

    #[tokio::test]
    async fn groups_load_in_order_key_order() {
        let (server, _dir) = server().await;
        let g1 = server.ensure_playlist_groups(None, 0).await.unwrap();
        let g2 = server
            .create_playlist_group("G2".into(), 0, OrderKey::greater(&OrderKey::wrap(vec![1])))
            .await
            .unwrap();
        let g3 = server
            .create_playlist_group("G3".into(), 0, OrderKey::greater(&OrderKey::wrap(vec![2])))
            .await
            .unwrap();

        // Move G1 after G3.
        server
            .set_playlist_group_order(g1, OrderKey::greater(&OrderKey::wrap(vec![3])))
            .await
            .unwrap();

        let groups = server.load_playlist_groups().await.unwrap();
        assert_eq!(
            groups.iter().map(|g| g.id).collect::<Vec<_>>(),
            vec![g2, g3, g1]
        );
    }
}
