//! `ease.library` JS bridge — read-only host library access for plugins.
//!
//! Exposes the host's playlists (id, title, member music ids) so data-style
//! views (e.g. the Play Counts plugin) can scope their own stored data to a
//! playlist without a bespoke host API per plugin, and music cover art as
//! engine image resources (`covers`) so views can show thumbnails without
//! pixel bytes ever entering the JS realm. Read-only global info:
//! unlike `ease.db` / `ease.secret`, calls are **not** plugin-scoped and
//! resolve no `PluginId` — there is nothing secret in a playlist roster or
//! an album cover.
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
use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use tur_engine::core::js_runtime::helpers::{extract_js_ctx, FnEntry, Ptr};

use ease_client_backend::error::BResult;
use ease_client_backend::repositories::core::DatabaseServer;
use ease_client_schema::MusicId;

/// Build the `FnEntry` table for the `library` namespace object.
pub fn build_fns() -> Vec<FnEntry> {
    vec![
        ("playlists", 0, playlists as Ptr),
        ("covers", 1, covers as Ptr),
    ]
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

// ---------------------------------------------------------------------------
// covers — music cover art as engine image resources
// ---------------------------------------------------------------------------

/// Thumbnail long edge. Covers render at 44 px in plugin lists; 2× covers
/// high-density screens while keeping a decoded thumbnail at ~30 KB RGBA
/// instead of the multi-megabyte full-resolution decode (covers are capped
/// at 4 MiB *encoded* — a 1400×1400 PNG is 7.8 MB as RGBA).
const COVER_THUMB_EDGE: u32 = 88;

/// Per-call batch cap. Thumbnails decode + register synchronously on the
/// calling instance's worker lane, so an unbounded request would stall the
/// plugin's own JS thread for the decodes; callers fetch the visible top of
/// their list, and anything past the cap is (warn-logged and) dropped.
const COVER_MAX_BATCH: usize = 64;

/// Decode cover bytes and downscale to a ≤[`COVER_THUMB_EDGE`] thumbnail
/// (box filter — `image`'s fast path). `None` = not decodable here (the
/// `image` crate's feature set is png/jpeg/webp; e.g. HEIC covers fall
/// through and render the plugin's fallback art) or not an image at all —
/// the write path sniffs covers, but be defensive anyway.
fn decode_cover_thumbnail(bytes: &[u8]) -> Option<tur_engine::ImageResource> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width() <= COVER_THUMB_EDGE && img.height() <= COVER_THUMB_EDGE {
        img
    } else {
        img.thumbnail(COVER_THUMB_EDGE, COVER_THUMB_EDGE)
    };
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    tur_engine::ImageResource::from_rgba(rgba.as_raw(), w, h)
}

/// `library.covers(musicIds: string[])` → `[{ musicId, id }]` — register
/// each requested music's cover as an engine image resource and return its
/// numeric resource id; `id` is `null` when the music row is gone, it has
/// no cover, or the bytes fail to decode. Pixel bytes never enter the JS
/// realm: the cover is decoded + thumbnailed right here (embedder-side
/// decode, the pattern tur #231 prescribes) and handed to
/// `TurInstanceContext::register_image`, which allocates the worker-side id
/// with its natural size recorded synchronously — so JS can immediately
/// wrap the id via `imageResourceHandle(id)` (validated) — and ships the
/// pixels to the host/GPU upload rail.
fn covers(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let cx = crate::plugin_runtime::backend_cx("library", args)?;
    let db: Arc<DatabaseServer> = cx.database_server().clone();

    let arr = JsArray::from_object(
        args.get_or_undefined(1)
            .as_object()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ()
                    .with_message("ease:library covers(musicIds) expects an array of id strings"))
            })?,
    )
    .map_err(|_| {
        JsError::from(JsNativeError::typ()
            .with_message("ease:library covers(musicIds) expects an array of id strings"))
    })?;

    let len = arr.length(ctx).unwrap_or(0) as usize;
    if len > COVER_MAX_BATCH {
        tracing::warn!(
            "ease:library covers: {} ids requested — truncating to {COVER_MAX_BATCH}",
            len
        );
    }
    let take = len.min(COVER_MAX_BATCH);
    let mut ids: Vec<i64> = Vec::with_capacity(take);
    for i in 0..take {
        let v = arr.at(i as i64, ctx)?;
        let s = v.as_string().ok_or_else(|| {
            JsError::from(JsNativeError::typ()
                .with_message("ease:library covers: musicIds entries must be strings"))
        })?;
        let id: i64 = s.to_std_string_escaped().parse().map_err(|_| {
            JsError::from(JsNativeError::typ()
                .with_message("ease:library covers: musicIds entries must be numeric id strings"))
        })?;
        ids.push(id);
    }

    // Blob reads (SQLite row + file blob) on the shared runtime, mirroring
    // the playlists fn's block_on discipline — each round-trip is ms-scale.
    let entries = ease_client_tokio::tokio_runtime().block_on(async move {
        let mut out: Vec<(i64, Option<Vec<u8>>)> = Vec::with_capacity(ids.len());
        for id in ids {
            // `None` = music row gone, no cover set, or the blob read
            // failed — all surface as `id: null` in JS (a missing cover is
            // a normal state, not an error).
            let cover = async {
                let music = db
                    .load_music(MusicId::wrap(id))
                    .await
                    .ok()
                    .flatten()?;
                let blob_id = music.cover?;
                db.blob().read(blob_id).ok()
            }
            .await;
            out.push((id, cover));
        }
        out
    });

    let out_arr = JsArray::new(ctx)?;
    for (i, (id, bytes)) in entries.into_iter().enumerate() {
        let resource_id = bytes
            .as_deref()
            .and_then(decode_cover_thumbnail)
            .map(|image| js_ctx.register_image(image).as_u64());
        let o = JsObject::with_object_proto(ctx.intrinsics());
        let id_s = id.to_string();
        o.create_data_property(
            js_string!("musicId"),
            JsValue::from(js_string!(id_s.as_str())),
            ctx,
        )?;
        let id_value = match resource_id {
            Some(v) => JsValue::from(v as f64),
            None => JsValue::null(),
        };
        o.create_data_property(js_string!("id"), id_value, ctx)?;
        let o_value: JsValue = o.into();
        out_arr.set(i as u32, o_value, true, ctx)?;
    }
    Ok(out_arr.into())
}
