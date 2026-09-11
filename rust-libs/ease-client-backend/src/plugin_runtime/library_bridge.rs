//! `ease.library` JS bridge — read-only host library access for plugins.
//!
//! Exposes the host's playlists (id, title, member music ids) so data-style
//! views (e.g. the Play Counts plugin) can scope their own stored data to a
//! playlist without a bespoke host API per plugin. Read-only global info:
//! unlike `ease.db` / `ease.secret`, calls are **not** plugin-scoped and
//! resolve no `PluginId` — there is nothing secret in a playlist roster.
//!
//! Like the db bridge, all operations run via `tokio_runtime().block_on(...)`
//! because tur's boa engine is single-threaded and `!Send`. Each call blocks
//! the engine thread for the two SQLite round-trips (~ms-scale).
//!
//! Bridge fns are ctx-bound (`FnEntry`): `bound_native` prepends the
//! per-instance `TurInstanceContext` to args (so JS args start at index 1),
//! but the identity slot is deliberately unread here.

use std::collections::HashMap;
use std::sync::Arc;

use boa_engine::object::builtins::JsArray;
use boa_engine::object::JsObject;
use boa_engine::{js_string, JsError, JsNativeError, JsResult, JsValue};
use tur_engine::core::js_runtime::helpers::{FnEntry, Ptr};

use crate::error::BResult;
use crate::repositories::core::DatabaseServer;

/// Build the `FnEntry` table for the `library` namespace object.
pub fn build_fns() -> Vec<FnEntry> {
    vec![("playlists", 0, playlists as Ptr)]
}

/// One playlist snapshot crossing the JS boundary.
struct PlaylistSnapshot {
    id: i64,
    title: String,
    music_ids: Vec<i64>,
}

// ---------------------------------------------------------------------------
// bridge fns
// ---------------------------------------------------------------------------

/// `library.playlists()` → `[{ id, title, musicIds }]` in the app's playlist
/// order. Ids are stringified i64 row ids — the same form `music:play` event
/// payloads carry (`MusicId.value`), so plugins match members against their
/// own stored rows with a plain string comparison.
fn playlists(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let cx = crate::plugin_runtime::backend_cx("library", args)?;
    let db: Arc<DatabaseServer> = cx.database_server().clone();

    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        let playlists = db.load_playlists().await?;
        let edges = db.load_all_playlist_music_edges().await?;

        let mut grouped: HashMap<i64, Vec<i64>> = HashMap::new();
        for edge in edges {
            grouped
                .entry(*edge.playlist_id.as_ref())
                .or_default()
                .push(*edge.music_id.as_ref());
        }

        let snapshots = playlists
            .into_iter()
            .map(|p| {
                let id = *p.id.as_ref();
                PlaylistSnapshot {
                    id,
                    title: p.title,
                    music_ids: grouped.remove(&id).unwrap_or_default(),
                }
            })
            .collect::<Vec<_>>();
        BResult::Ok(snapshots)
    });
    let snapshots = result.map_err(|e| {
        JsError::from(
            JsNativeError::typ().with_message(format!("ease:library playlists failed: {e:?}")),
        )
    })?;

    let arr = JsArray::new(ctx)?;
    for (i, p) in snapshots.iter().enumerate() {
        let o = JsObject::with_object_proto(ctx.intrinsics());

        let id_s = p.id.to_string();
        o.create_data_property(
            js_string!("id"),
            JsValue::from(js_string!(id_s.as_str())),
            ctx,
        )?;
        o.create_data_property(
            js_string!("title"),
            JsValue::from(js_string!(p.title.as_str())),
            ctx,
        )?;

        let ids = JsArray::new(ctx)?;
        for (j, mid) in p.music_ids.iter().enumerate() {
            let s = mid.to_string();
            ids.set(j as u32, JsValue::from(js_string!(s.as_str())), true, ctx)?;
        }
        let ids_value: JsValue = ids.into();
        o.create_data_property(js_string!("musicIds"), ids_value, ctx)?;

        let o_value: JsValue = o.into();
        arr.set(i as u32, o_value, true, ctx)?;
    }
    Ok(arr.into())
}
