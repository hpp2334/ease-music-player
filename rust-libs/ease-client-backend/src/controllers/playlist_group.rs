use ease_client_schema::{PlaylistGroupId, PlaylistId};
use ease_client_tokio::tokio_runtime;
use ease_order_key::{OrderKey, OrderKeyRef};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    ctx::BackendContext,
    error::{BError, BResult},
    objects::PlaylistGroupMeta,
    services::get_all_playlist_groups,
    Backend,
};

pub async fn ct_list_playlist_groups(cx: Arc<Backend>) -> BResult<Vec<PlaylistGroupMeta>> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            get_all_playlist_groups(cx).await
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgCreatePlaylistGroup {
    pub title: String,
}

pub async fn ct_create_playlist_group(
    cx: Arc<Backend>,
    arg: ArgCreatePlaylistGroup,
) -> BResult<PlaylistGroupId> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            let current_time_ms = cx.current_time().as_millis() as i64;

            let last_order = get_all_playlist_groups(cx)
                .await?
                .last()
                .map(|v| OrderKey::wrap(v.order.clone()))
                .unwrap_or_default();

            cx.database_server()
                .create_playlist_group(arg.title, current_time_ms, OrderKey::greater(&last_order))
                .await
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgUpdatePlaylistGroup {
    pub id: PlaylistGroupId,
    pub title: String,
}

pub async fn ct_update_playlist_group(
    cx: Arc<Backend>,
    arg: ArgUpdatePlaylistGroup,
) -> BResult<()> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            cx.database_server()
                .update_playlist_group(arg.id, arg.title)
                .await?;
            Ok(())
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgRemovePlaylistGroup {
    pub id: PlaylistGroupId,
    /// `false` (default) = sweep the group's playlists into the first
    /// remaining group; `true` = remove the playlists together with
    /// the group (full playlist-removal cascade).
    #[serde(default)]
    pub delete_playlists: bool,
}

pub async fn ct_remove_playlist_group(
    cx: Arc<Backend>,
    arg: ArgRemovePlaylistGroup,
) -> BResult<()> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            cx.database_server()
                .remove_playlist_group(arg.id, arg.delete_playlists)
                .await?;
            Ok(())
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgSetPlaylistGroupExpanded {
    pub id: PlaylistGroupId,
    pub expanded: bool,
}

pub async fn ct_set_playlist_group_expanded(
    cx: Arc<Backend>,
    arg: ArgSetPlaylistGroupExpanded,
) -> BResult<()> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            cx.database_server()
                .set_playlist_group_expanded(arg.id, arg.expanded)
                .await?;
            Ok(())
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgEnsurePlaylistGroups {
    /// Localized title for the group created when none exists (the
    /// Kotlin caller passes its resolved-locale "Default"). Falls back
    /// to "Default" Rust-side.
    #[serde(default)]
    pub default_title: Option<String>,
}

/// The "always create a Default group" self-heal: creates a group when
/// zero exist and sweeps orphaned playlists into the first group.
/// Idempotent.
pub async fn ct_ensure_playlist_groups(
    cx: Arc<Backend>,
    arg: ArgEnsurePlaylistGroups,
) -> BResult<PlaylistGroupId> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            let current_time_ms = cx.current_time().as_millis() as i64;
            cx.database_server()
                .ensure_playlist_groups(arg.default_title, current_time_ms)
                .await
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgMovePlaylistToGroup {
    pub playlist_id: PlaylistId,
    pub group_id: PlaylistGroupId,
    /// Destination neighbors for the landing position (`a` = previous,
    /// `b` = next, both within `group_id`; `None` = edge/empty group).
    pub a: Option<PlaylistId>,
    pub b: Option<PlaylistId>,
}

/// Drag-drop move of a playlist into another group at a specific
/// position (group change + order derived from destination neighbors,
/// in one write).
pub async fn ct_move_playlist_to_group(
    cx: Arc<Backend>,
    arg: ArgMovePlaylistToGroup,
) -> BResult<()> {
    tokio_runtime()
        .handle()
        .spawn(async move {
            let cx = cx.get_context();
            cx.database_server()
                .move_playlist_to_group_at(arg.playlist_id, arg.group_id, arg.a, arg.b)
                .await?;
            Ok(())
        })
        .await
        .unwrap()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgReorderPlaylistGroup {
    pub id: PlaylistGroupId,
    pub a: Option<PlaylistGroupId>,
    pub b: Option<PlaylistGroupId>,
}

pub fn cts_reorder_playlist_group(cx: Arc<Backend>, arg: ArgReorderPlaylistGroup) -> BResult<()> {
    let cx = cx.get_context().clone();
    tokio_runtime().block_on(async move { reorder_playlist_group_inner(&cx, arg).await })
}

pub(crate) async fn reorder_playlist_group_inner(
    cx: &BackendContext,
    arg: ArgReorderPlaylistGroup,
) -> BResult<()> {
    if arg.a == arg.b {
        return Ok(());
    }

    let groups = get_all_playlist_groups(cx).await?;

    let from = groups
        .iter()
        .find(|v| v.id == arg.id)
        .ok_or(BError::GroupNotFound(arg.id))?;
    let a = match arg.a {
        Some(id) => Some(
            groups
                .iter()
                .find(|v| v.id == id)
                .ok_or(BError::GroupNotFound(id))?,
        ),
        None => None,
    };
    let b = match arg.b {
        Some(id) => Some(
            groups
                .iter()
                .find(|v| v.id == id)
                .ok_or(BError::GroupNotFound(id))?,
        ),
        None => None,
    };

    if a.is_none() && b.is_none() {
        tracing::warn!("reorder but both playlist groups are null");
        return Ok(());
    }

    let a_order = a.map(|v| OrderKeyRef::wrap(&v.order));
    let b_order = b.map(|v| OrderKeyRef::wrap(&v.order));
    let order = {
        match (a_order, b_order) {
            (Some(a), Some(b)) => OrderKey::between(a, b)?,
            (Some(a), None) => OrderKey::greater(a),
            (None, Some(b)) => OrderKey::less_or_fallback(b),
            (None, None) => unreachable!(),
        }
    };

    cx.database_server()
        .set_playlist_group_order(from.id, order)
        .await?;
    Ok(())
}
