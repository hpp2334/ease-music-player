//! The `ease` service rows — the host services plugins reach from rut.
//!
//! One `push_rut_ext` installer covering six namespaces (the `ease.rpc` bus
//! rows live in `ease-tur-rpc`'s [`ease_tur_rpc::TurRpcBusPlugin`] plus the
//! `call` row registered here; the `crypto` / `xml` / `net` / `time`
//! utilities live in [`super::util_rows`]):
//!
//! - `ease.db` — plugin-scoped KV over [`DatabaseServer`] (JSON in/out for
//!   the composite ops, `?str` for nullable gets).
//! - `ease.secret` — plugin-scoped secret store (`SecretScope::Plugin`).
//! - `ease.oauth` — flow-id mint + the Kotlin OAuth upcall.
//! - `ease.themes` — the host theme colors (`#RRGGBBAA` strings) + dark.
//! - `ease.context` — storage-row lifecycle (`notify_change` /
//!   `create_storage` / `remove_storage`) + the instance's `storage_id`.
//! - `ease.library` — read-only playlist roster + cover thumbnails as
//!   engine image resources (pixel bytes never enter the script).
//! - `ease.rpc.call` — a view calling its own backend's bus.
//!
//! Identity: every scoped row resolves the calling plugin from the
//! per-instance [`PluginId`] data slot — never from a rut argument.
//! Blocking: DB round-trips `block_on` the shared tokio runtime (ms-scale,
//! the old bridges' discipline); storage-lifecycle mutations spawn instead
//! (they can chain upcalls and must never re-enter the pump).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ease_client_backend::error::BResult;
use ease_client_backend::repositories::core::DatabaseServer;
use ease_client_backend::repositories::secret::SecretStore;
use ease_client_schema::{MusicId, PluginKvEntry, SecretId, SecretScope};
use rut_core::types::{TY_BOOL, TY_I64, TY_NIL, TY_OPAQUE, TY_OPT_STR, TY_STR, TY_U64};
use rut_vm::TrapKind;
use tur_engine::core::rut_runtime::{host_row_at, RutHandles, RutPkgCx};

use crate::plugin_runtime::{backend_cx, plugin_id, rpc_bus_cell, PluginId};

/// OAuth flow-id sequence (host keeps no state — the plugin keys its flow
/// data by the id in its own KV).
static OAUTH_FLOW_SEQ: AtomicU64 = AtomicU64::new(1);

/// The installer. Called twice by the engine (decl probe, then boot).
pub fn install(cx: &mut RutPkgCx<'_>) {
    let row = |path: &str, n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId, a: bool| {
        host_row_at(path, n, p, r, a)
    };
    cx.decl.extend(
        vec![
            // ---- db (plugin-scoped KV). Composites ride JSON strings. ----
            row("ease/db", "single_get", vec![TY_STR], TY_OPT_STR, false),
            row("ease/db", "single_get_multi", vec![TY_STR], TY_STR, false),
            row("ease/db", "single_set", vec![TY_STR, TY_STR], TY_NIL, false),
            row("ease/db", "single_set_multi", vec![TY_STR], TY_NIL, false),
            row("ease/db", "single_delete", vec![TY_STR], TY_NIL, false),
            row("ease/db", "single_delete_multi", vec![TY_STR], TY_NIL, false),
            row("ease/db", "multi_append", vec![TY_STR, TY_STR], TY_NIL, false),
            row("ease/db", "multi_append_multi", vec![TY_STR], TY_NIL, false),
            row("ease/db", "multi_get_all", vec![TY_STR], TY_STR, false),
            row("ease/db", "multi_get_all_multi", vec![TY_STR], TY_STR, false),
            row("ease/db", "multi_count", vec![TY_STR], TY_U64, false),
            row("ease/db", "multi_count_multi", vec![TY_STR], TY_STR, false),
            row("ease/db", "multi_delete", vec![TY_STR], TY_NIL, false),
            row("ease/db", "multi_delete_multi", vec![TY_STR], TY_NIL, false),
            row("ease/db", "list_keys", vec![TY_STR], TY_STR, false),
            // ---- secret (plugin-scoped secret store) ----
            row("ease/secret", "get", vec![TY_I64], TY_OPT_STR, false),
            row("ease/secret", "put", vec![TY_STR], TY_I64, false),
            row("ease/secret", "remove", vec![TY_I64], TY_NIL, false),
            // ---- oauth (flow mint + Kotlin upcall) ----
            row("ease/oauth", "new", vec![], TY_STR, false),
            row("ease/oauth", "begin", vec![TY_STR], TY_NIL, false),
            // ---- themes (host palette) ----
            row("ease/themes", "color", vec![TY_STR], TY_STR, false),
            row("ease/themes", "is_dark", vec![], TY_BOOL, false),
            // ---- context (storage lifecycle) ----
            row("ease/context", "notify_change", vec![], TY_NIL, false),
            row("ease/context", "create_storage", vec![TY_STR], TY_NIL, false),
            row("ease/context", "remove_storage", vec![TY_STR], TY_NIL, false),
            row("ease/context", "storage_id", vec![], TY_OPT_STR, false),
            // ---- library (read-only host data) ----
            row("ease/library", "playlists", vec![], TY_STR, false),
            row("ease/library", "covers", vec![TY_STR], TY_STR, false),
            // ---- rpc (view → its own backend; the envelope passes through) ----
            row("ease/rpc", "call", vec![TY_STR, TY_STR], TY_STR, false),
        ]
        .into_iter(),
    );

    let Some(handles) = cx.handles else {
        return; // compile-time decl probe — bodies install at boot only
    };
    let handles = handles.clone();
    let pkg = &mut *cx.pkg;

    /// One decl-row binding: clones the per-instance handles into the
    /// closure (each row owns its `Rc`) and rebinds the call-site `handles`
    /// ident to the clone inside the body.
    macro_rules! row_fn {
        ($handles:ident, $key:literal, ($($arg:ident: $ty:ty),*) -> $ret:ty, $($body:tt)*) => {{
            let h = $handles.clone();
            rut_vm::pkg_fn!(pkg, $key, ($($ty,)*) -> $ret, move |vm: &mut rut_vm::interp::Vm, $($arg: $ty),*| {
                #[allow(unused_variables)]
                let $handles = &h;
                #[allow(unused_variables)]
                let _ = vm;
                $($body)*
            })
        }};
    }

    // ---- db ---------------------------------------------------------------

    row_fn!(handles, "ease::db::single_get", (key: &str) -> Option<String>, {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let key = key.to_string();
        let v = block_on("single_get", db.plugin_kv_single_get(pid.as_str(), &key))?;
        Ok(v)
    });

    row_fn!(handles, "ease::db::single_get_multi", (keys_json: &str) -> String, {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let keys: Vec<String> = parse_json(keys_json, "keys")?;
        let rows = block_on("single_get_multi", db.plugin_kv_single_get_multi(pid.as_str(), keys))?;
        Ok(serde_json::to_string(&rows).expect("entry rows serialize"))
    });

    row_fn!(handles, "ease::db::single_set", (key: &str, value: &str) -> (), {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        block_on("single_set", db.plugin_kv_single_set(pid.as_str(), key, value))?;
        Ok(())
    });

    row_fn!(handles, "ease::db::single_set_multi", (entries_json: &str) -> (), {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let entries: Vec<PluginKvEntry> = parse_json(entries_json, "entries")?;
        block_on("single_set_multi", db.plugin_kv_single_set_multi(pid.as_str(), entries))?;
        Ok(())
    });

    row_fn!(handles, "ease::db::single_delete", (key: &str) -> (), {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        block_on("single_delete", db.plugin_kv_single_delete(pid.as_str(), key))?;
        Ok(())
    });

    row_fn!(handles, "ease::db::single_delete_multi", (keys_json: &str) -> (), {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let keys: Vec<String> = parse_json(keys_json, "keys")?;
        block_on("single_delete_multi", db.plugin_kv_single_delete_multi(pid.as_str(), keys))?;
        Ok(())
    });

    row_fn!(handles, "ease::db::multi_append", (key: &str, value: &str) -> (), {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        block_on("multi_append", db.plugin_kv_multi_append(pid.as_str(), key, value))?;
        Ok(())
    });

    row_fn!(handles, "ease::db::multi_append_multi", (entries_json: &str) -> (), {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let entries: Vec<PluginKvEntry> = parse_json(entries_json, "entries")?;
        block_on("multi_append_multi", db.plugin_kv_multi_append_multi(pid.as_str(), entries))?;
        Ok(())
    });

    row_fn!(handles, "ease::db::multi_get_all", (key: &str) -> String, {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let values = block_on("multi_get_all", db.plugin_kv_multi_get_all(pid.as_str(), key))?;
        Ok(serde_json::to_string(&values).expect("values serialize"))
    });

    row_fn!(handles, "ease::db::multi_get_all_multi", (keys_json: &str) -> String, {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let keys: Vec<String> = parse_json(keys_json, "keys")?;
        let rows = block_on("multi_get_all_multi", db.plugin_kv_multi_get_all_multi(pid.as_str(), keys))?;
        Ok(serde_json::to_string(&rows).expect("multi rows serialize"))
    });

    row_fn!(handles, "ease::db::multi_count", (key: &str) -> u64, {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let n = block_on("multi_count", db.plugin_kv_multi_count(pid.as_str(), key))?;
        Ok(n as u64)
    });

    row_fn!(handles, "ease::db::multi_count_multi", (keys_json: &str) -> String, {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let keys: Vec<String> = parse_json(keys_json, "keys")?;
        let rows = block_on("multi_count_multi", db.plugin_kv_multi_count_multi(pid.as_str(), keys))?;
        Ok(serde_json::to_string(&rows).expect("count rows serialize"))
    });

    row_fn!(handles, "ease::db::multi_delete", (key: &str) -> (), {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        block_on("multi_delete", db.plugin_kv_multi_delete(pid.as_str(), key))?;
        Ok(())
    });

    row_fn!(handles, "ease::db::multi_delete_multi", (keys_json: &str) -> (), {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let keys: Vec<String> = parse_json(keys_json, "keys")?;
        block_on("multi_delete_multi", db.plugin_kv_multi_delete_multi(pid.as_str(), keys))?;
        Ok(())
    });

    row_fn!(handles, "ease::db::list_keys", (prefix: &str) -> String, {
        let pid = plugin_id("db", &handles)?;
        let db = db_of(&handles)?;
        let rows = block_on("list_keys", db.plugin_kv_list_keys(pid.as_str(), prefix))?;
        let out: Vec<serde_json::Value> = rows
            .iter()
            .map(|info| {
                serde_json::json!({ "key": info.key, "kind": info.kind.as_i32() })
            })
            .collect();
        Ok(serde_json::to_string(&out).expect("key infos serialize"))
    });

    // ---- secret -------------------------------------------------------------

    row_fn!(handles, "ease::secret::get", (id: i64) -> Option<String>, {
        let (pid, db) = secret_parts(&handles)?;
        let v = block_on("get", db.secret_get(scope_of(&pid), SecretId::wrap(id)))?;
        Ok(v)
    });

    row_fn!(handles, "ease::secret::put", (secret: &str) -> i64, {
        let (pid, db) = secret_parts(&handles)?;
        let secret = secret.to_string();
        let id = block_on("put", db.secret_put(scope_of(&pid), secret))?;
        Ok(*id.as_ref())
    });

    row_fn!(handles, "ease::secret::remove", (id: i64) -> (), {
        let (pid, db) = secret_parts(&handles)?;
        block_on("remove", db.secret_remove(scope_of(&pid), SecretId::wrap(id)))?;
        Ok(())
    });

    // ---- oauth ---------------------------------------------------------------

    row_fn!(handles, "ease::oauth::new", () -> String, {
        plugin_id("oauth", &handles)?; // gate to plugin-bound instances
        let n = OAUTH_FLOW_SEQ.fetch_add(1, Ordering::Relaxed);
        Ok(format!("oauth-{n}"))
    });

    row_fn!(handles, "ease::oauth::begin", (oauth_id: &str) -> (), {
        let pid = plugin_id("oauth", &handles)?;
        let oauth_id = oauth_id.to_string();
        #[cfg(target_os = "android")]
        {
            if let Err(e) = upcall_start_oauth(pid.as_ref(), &oauth_id) {
                tracing::warn!("ease:oauth.begin({oauth_id}) upcall failed: {e}");
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (pid.as_ref(), oauth_id);
            tracing::debug!("ease:oauth.begin ignored (non-Android target)");
        }
        Ok(())
    });

    // ---- themes ---------------------------------------------------------------

    row_fn!(handles, "ease::themes::color", (name: &str) -> String, {
        let _ = plugin_id("themes", &handles)?;
        let name = name.to_string();
        #[cfg(target_os = "android")]
        {
            theme_upcall_color(&name).map_err(|e| {
                rut_vm::Trap::new(
                    TrapKind::Invalid,
                    format!("ease:themes.color: host upcall failed: {e}"),
                )
            })
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = name;
            Err(rut_vm::Trap::new(
                TrapKind::Invalid,
                "ease:themes.color: theme is only available on Android",
            ))
        }
    });

    row_fn!(handles, "ease::themes::is_dark", () -> bool, {
        let _ = plugin_id("themes", &handles)?;
        #[cfg(target_os = "android")]
        {
            // Upcall failure degrades to the light default (the old
            // bridge's `unwrap_or(false)`), never a trap.
            Ok(theme_upcall_is_dark().unwrap_or(false))
        }
        #[cfg(not(target_os = "android"))]
        {
            Err(rut_vm::Trap::new(
                TrapKind::Invalid,
                "ease:themes.is_dark: theme is only available on Android",
            ))
        }
    });

    // ---- context --------------------------------------------------------------

    row_fn!(handles, "ease::context::notify_change", () -> (), {
        let (pid, inst) = resolve_instance(&handles)?;
        let instance_str = inst.0.unwrap_or_default();
        #[cfg(target_os = "android")]
        {
            if let Err(e) = upcall_storage_host(&pid, &instance_str, "notifyChange") {
                tracing::warn!("ease:context.notify_change upcall failed: {e}");
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (pid, instance_str);
            tracing::debug!("ease:context.notify_change ignored (non-Android target)");
        }
        Ok(())
    });

    row_fn!(handles, "ease::context::create_storage", (plugin_storage_id: &str) -> (), {
        use ease_client_schema::{PluginId as Pid, PluginStorageId as Psid, StorageHandle};

        let pid = plugin_id("context", &handles)?;
        let cx = backend_cx("context", &handles)?;
        let plugin_storage_id = plugin_storage_id.to_string();
        let pid_str = pid.as_str().to_string();
        let instance = plugin_storage_id.clone();

        // Spawned (never block_on on the worker): the DB work chains the
        // Kotlin upcall, so ordering is preserved without re-entering the
        // pump.
        ease_client_tokio::tokio_runtime().spawn(async move {
            let handle = StorageHandle::Plugin {
                plugin_id: Pid::new(pid_str.clone()),
                plugin_storage_id: Psid::new(instance.clone()),
            };
            let result = cx
                .database_server()
                .obtain_storage(&handle)
                .await
                .map(|id| {
                    ease_client_backend::services::evict_storage_backend_cache(&cx, id);
                    id
                });
            match result {
                Ok(id) => {
                    tracing::debug!(
                        "create_storage: registered {instance} as storage {:?}",
                        id.as_ref()
                    );
                    #[cfg(target_os = "android")]
                    {
                        if let Err(e) = upcall_storage_host(&pid_str, &instance, "storageCreated") {
                            tracing::warn!("ease:context.create_storage upcall failed: {e}");
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("ease:context.create_storage failed for {instance}: {e:?}");
                }
            }
        });
        Ok(())
    });

    row_fn!(handles, "ease::context::remove_storage", (plugin_storage_id: &str) -> (), {
        use ease_client_schema::StorageId;

        let pid = plugin_id("context", &handles)?;
        let cx = backend_cx("context", &handles)?;
        let plugin_storage_id = plugin_storage_id.to_string();
        let pid_str = pid.as_str().to_string();

        ease_client_tokio::tokio_runtime().spawn(async move {
            let rows = cx.database_server().load_all_storage_rows().await?;
            let id = rows
                .into_iter()
                .find(|r| {
                    r.plugin_id.as_deref() == Some(pid_str.as_str())
                        && r.plugin_storage_id.as_deref() == Some(plugin_storage_id.as_str())
                })
                .map(|r| StorageId::wrap(r.id));
            if let Some(id) = id {
                ease_client_backend::services::storage::remove_storage(&cx, id).await?;
            }
            Ok::<_, ease_client_backend::error::BError>(())
        });
        Ok(())
    });

    row_fn!(handles, "ease::context::storage_id", () -> Option<String>, {
        // The instance's storage identity: `nil` = create-mode view or the
        // headless backend, `Some` = an edit-mode view. Never changes for
        // the instance lifetime — read once at start.
        Ok(handles
            .inst
            .data::<crate::plugin_runtime::PluginInstance>()
            .and_then(|i| i.0))
    });

    // ---- library ---------------------------------------------------------------

    row_fn!(handles, "ease::library::playlists", () -> String, {
        let cx = backend_cx("library", &handles)?;
        let db: Arc<DatabaseServer> = cx.database_server().clone();
        let result: BResult<Vec<serde_json::Value>> = block_on_async("playlists", async move {
            let playlists = db.load_playlists().await?;
            let edges = db.load_all_playlist_music_edges().await?;

            let mut grouped: HashMap<i64, Vec<i64>> = HashMap::new();
            for edge in edges {
                grouped
                    .entry(*edge.playlist_id.as_ref())
                    .or_default()
                    .push(*edge.music_id.as_ref());
            }

            let out = playlists
                .into_iter()
                .map(|p| {
                    let id = *p.id.as_ref();
                    let music_ids: Vec<String> = grouped
                        .remove(&id)
                        .unwrap_or_default()
                        .iter()
                        .map(|m| m.to_string())
                        .collect();
                    Ok(serde_json::json!({
                        "id": id.to_string(),
                        "title": p.title,
                        "musicIds": music_ids,
                    }))
                })
                .collect::<BResult<Vec<_>>>();
            Ok::<_, ease_client_backend::error::BError>(out)
        })?;
        Ok(serde_json::to_string(&result).expect("playlists serialize"))
    });

    row_fn!(handles, "ease::library::covers", (music_ids_json: &str) -> String, {
        Ok(covers_row(&handles, music_ids_json)?)
    });

    // ---- rpc.call (view → its own backend) --------------------------------------

    row_fn!(handles, "ease::rpc::call", (op: &str, args_json: &str) -> String, {
        Ok(rpc_call_row(&handles, op, args_json)?)
    });
}

// ---------------------------------------------------------------------------
// shared row helpers
// ---------------------------------------------------------------------------

/// The context-bridge identity pair (plugin id + storage instance).
fn resolve_instance(
    handles: &std::rc::Rc<RutHandles>,
) -> Result<(PluginId, crate::plugin_runtime::PluginInstance), rut_vm::Trap> {
    let pid = plugin_id("context", handles)?;
    let inst = handles
        .inst
        .data::<crate::plugin_runtime::PluginInstance>()
        .unwrap_or(crate::plugin_runtime::PluginInstance(None));
    Ok((pid, inst))
}

fn db_of(
    handles: &std::rc::Rc<RutHandles>,
) -> Result<Arc<DatabaseServer>, rut_vm::Trap> {
    Ok(backend_cx("db", handles)?.database_server().clone())
}

fn secret_parts(
    handles: &std::rc::Rc<RutHandles>,
) -> Result<(PluginId, Arc<DatabaseServer>), rut_vm::Trap> {
    let pid = plugin_id("secret", handles)?;
    let db = backend_cx("secret", handles)?.database_server().clone();
    Ok((pid, db))
}

fn scope_of(pid: &PluginId) -> SecretScope {
    SecretScope::Plugin(ease_client_schema::PluginId::new(pid.as_str()))
}

fn parse_json<T: serde::de::DeserializeOwned>(s: &str, what: &str) -> Result<T, rut_vm::Trap> {
    serde_json::from_str(s).map_err(|e| {
        rut_vm::Trap::new(
            TrapKind::Invalid,
            format!("ease:db: {what} is not valid JSON: {e}"),
        )
    })
}

/// Block on the shared tokio runtime. Rows run on a worker lane; the tokio
/// runtime is a separate driver, so this is the old bridges' exact
/// discipline (ms-scale round-trips only).
fn block_on<R>(what: &str, fut: impl std::future::Future<Output = BResult<R>>) -> Result<R, rut_vm::Trap> {
    ease_client_tokio::tokio_runtime()
        .block_on(fut)
        .map_err(|e| {
            rut_vm::Trap::new(
                TrapKind::Invalid,
                format!("ease:db {what} failed: {e:?}"),
            )
        })
}

/// [`block_on`] over a future that is already `Result`-shaped.
fn block_on_async<R, E: std::fmt::Debug>(
    what: &str,
    fut: impl std::future::Future<Output = Result<R, E>>,
) -> Result<R, rut_vm::Trap> {
    ease_client_tokio::tokio_runtime()
        .block_on(fut)
        .map_err(|e| {
            rut_vm::Trap::new(
                TrapKind::Invalid,
                format!("ease:db {what} failed: {e:?}"),
            )
        })
}

// ---------------------------------------------------------------------------
// covers
// ---------------------------------------------------------------------------

/// Thumbnail long edge. Covers render at 44 px in plugin lists; 2× covers
/// high-density screens while keeping a decoded thumbnail at ~30 KB RGBA
/// instead of the multi-megabyte full-resolution decode.
const COVER_THUMB_EDGE: u32 = 88;

/// Per-call batch cap (the caller fetches the visible top of its list).
const COVER_MAX_BATCH: usize = 64;

/// Decode cover bytes and downscale to a ≤[`COVER_THUMB_EDGE`] thumbnail.
/// `None` = not decodable here (png/jpeg/webp; HEIC falls through) or not
/// an image at all.
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

/// `covers(musicIdsJson: str) -> str` — `musicIds` is a JSON array of the
/// stringified music row ids; the answer is `[{"musicId": "...", "id": n | null}]`
/// where `id` is the registered engine image resource (bytes never enter
/// the script).
fn covers_row(
    handles: &std::rc::Rc<RutHandles>,
    music_ids_json: &str,
) -> Result<String, rut_vm::Trap> {
    let cx = backend_cx("library", handles)?;
    let db: Arc<DatabaseServer> = cx.database_server().clone();

    let ids: Vec<String> = parse_json(music_ids_json, "musicIds")?;
    if ids.len() > COVER_MAX_BATCH {
        tracing::warn!(
            "ease:library covers: {} ids requested — truncating to {COVER_MAX_BATCH}",
            ids.len()
        );
    }
    let mut ids: Vec<i64> = ids
        .into_iter()
        .take(COVER_MAX_BATCH)
        .map(|s| s.parse::<i64>())
        .collect::<Result<_, _>>()
        .map_err(|_| {
            rut_vm::Trap::new(
                TrapKind::Invalid,
                "ease:library covers: musicIds entries must be numeric id strings",
            )
        })?;

    let entries = ease_client_tokio::tokio_runtime().block_on(async move {
        let mut out: Vec<(i64, Option<Vec<u8>>)> = Vec::with_capacity(ids.len());
        for id in ids.drain(..) {
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

    let mut out: Vec<serde_json::Value> = Vec::with_capacity(entries.len());
    for (id, bytes) in entries {
        let resource_id = bytes
            .as_deref()
            .and_then(decode_cover_thumbnail)
            .map(|image| handles.inst.register_image(image).as_u64());
        out.push(serde_json::json!({
            "musicId": id.to_string(),
            "id": resource_id,
        }));
    }
    Ok(serde_json::to_string(&out).expect("covers serialize"))
}

// ---------------------------------------------------------------------------
// rpc.call (view → its own backend)
// ---------------------------------------------------------------------------

/// `call(op, argsJson) -> envelope str` — route a view's call to its
/// plugin's backend bus. Blocking (control calls are ms-scale; the view's
/// VM task waits on the backend's host-thread round-trip). The envelope
/// passes through: `{"ok":true,"result":…}` / `{"ok":false,"error":"…"}`.
fn rpc_call_row(
    handles: &std::rc::Rc<RutHandles>,
    op: &str,
    args_json: &str,
) -> Result<String, rut_vm::Trap> {
    let pid = plugin_id("rpc", handles)?;
    let cx = backend_cx("rpc", handles)?;
    let args: serde_json::Value = parse_json(args_json, "args")?;
    let Some(client) = cx.service_rpc_for(pid.as_str()) else {
        return Ok(serde_json::to_string(&ease_tur_rpc::err_envelope(
            "the plugin backend is not running",
        ))
        .expect("envelope serializes"));
    };
    let op = op.to_string();
    let reply = ease_client_tokio::tokio_runtime()
        .block_on(client.call_host(&op, args));
    match reply {
        Ok(v) => Ok(serde_json::to_string(&ease_tur_rpc::ok_envelope(v)).expect("envelope serializes")),
        Err(e) => Ok(serde_json::to_string(&ease_tur_rpc::err_envelope(&e.to_string()))
            .expect("envelope serializes")),
    }
}

// ---------------------------------------------------------------------------
// JNI upcalls (the Kotlin host statics)
// ---------------------------------------------------------------------------

/// Call a `(pluginId, instance) -> void` static on the cached
/// `EaseStorageHost` class (`notifyChange` / `storageCreated`).
#[cfg(target_os = "android")]
fn upcall_storage_host(plugin_id: &str, instance: &str, method: &str) -> Result<(), String> {
    use jni::objects::{JClass, JValue};

    let raw_vm = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    let vm = unsafe { jni::JavaVM::from_raw(raw_vm) }.map_err(|e| format!("from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;

    let raw_class =
        crate::plugin_runtime::host_cache::storage_host_class().ok_or("storage host class not cached")?;
    // SAFETY: process-lifetime global ref cached at createRuntime.
    let class = unsafe { JClass::from_raw(raw_class) };
    let pid_jstr = env
        .new_string(plugin_id)
        .map_err(|e| format!("new_string plugin_id: {e}"))?;
    let inst_jstr = env
        .new_string(instance)
        .map_err(|e| format!("new_string instance: {e}"))?;
    env.call_static_method(
        class,
        method,
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&pid_jstr), JValue::Object(&inst_jstr)],
    )
    .map_err(|e| format!("call_static_method: {e}"))?;
    Ok(())
}

/// `EaseOauthHost.startOauth(pluginId, oauthId)`.
#[cfg(target_os = "android")]
fn upcall_start_oauth(plugin_id: &str, oauth_id: &str) -> Result<(), String> {
    use jni::objects::{JClass, JValue};

    let raw_vm = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    let vm = unsafe { jni::JavaVM::from_raw(raw_vm) }.map_err(|e| format!("from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;

    let raw_class = crate::plugin_runtime::host_cache::oauth_host_class()
        .ok_or("oauth host class not cached")?;
    // SAFETY: process-lifetime global ref.
    let class = unsafe { JClass::from_raw(raw_class) };
    let plugin_jstr = env
        .new_string(plugin_id)
        .map_err(|e| format!("new_string plugin_id: {e}"))?;
    let oauth_jstr = env
        .new_string(oauth_id)
        .map_err(|e| format!("new_string oauth_id: {e}"))?;
    env.call_static_method(
        class,
        "startOauth",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&plugin_jstr), JValue::Object(&oauth_jstr)],
    )
    .map_err(|e| format!("call_static_method: {e}"))?;
    Ok(())
}

/// `EaseThemesHost.getColor(name) -> String`.
#[cfg(target_os = "android")]
fn theme_upcall_color(name: &str) -> Result<String, String> {
    use jni::objects::{JClass, JValue};

    let raw_vm = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    let vm = unsafe { jni::JavaVM::from_raw(raw_vm) }.map_err(|e| format!("from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;

    let raw_class = crate::plugin_runtime::host_cache::theme_host_class()
        .ok_or("theme host class not cached")?;
    // SAFETY: process-lifetime global ref.
    let class = unsafe { JClass::from_raw(raw_class) };
    let name_jstr = env
        .new_string(name)
        .map_err(|e| format!("new_string name: {e}"))?;
    let ret = env
        .call_static_method(
            class,
            "getColor",
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&name_jstr)],
        )
        .map_err(|e| format!("call_static_method: {e}"))?;
    let obj = ret.l().map_err(|e| format!("ret.l: {e}"))?;
    // Kotlin answers null for an unknown role. `get_string(null)` would
    // throw a JVM NPE that jni-rs reports WITHOUT clearing, poisoning the
    // worker lane's pending-exception state for later upcalls — null-check
    // and fail with a clean message instead (the old bridge's contract).
    if obj.is_null() {
        return Err(format!("unknown color name: {name}"));
    }
    let s: String = env
        .get_string((&obj).into())
        .map_err(|e| format!("get_string: {e}"))?
        .into();
    Ok(s)
}

/// `EaseThemesHost.isDark() -> boolean`.
#[cfg(target_os = "android")]
fn theme_upcall_is_dark() -> Result<bool, String> {
    use jni::objects::JClass;

    let raw_vm = ndk_context::android_context().vm() as *mut jni::sys::JavaVM;
    let vm = unsafe { jni::JavaVM::from_raw(raw_vm) }.map_err(|e| format!("from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;

    let raw_class = crate::plugin_runtime::host_cache::theme_host_class()
        .ok_or("theme host class not cached")?;
    // SAFETY: process-lifetime global ref.
    let class = unsafe { JClass::from_raw(raw_class) };
    let ret = env
        .call_static_method(class, "isDark", "()Z", &[])
        .map_err(|e| format!("call_static_method: {e}"))?;
    Ok(ret.z().map_err(|e| format!("ret.z: {e}"))?)
}
