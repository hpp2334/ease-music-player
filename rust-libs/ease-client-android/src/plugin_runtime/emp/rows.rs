//! `emp_host` rows — the service surface behind the `emp` classes.
//! Same per-op work as the old `ease` rows ([`super::super::
//! service_rows`] — the `op_*` helpers are shared), with the identity
//! law flipped: every scoped row carries a LEADING `opaque` parameter
//! (the host-minted [`PluginHandle`]) instead of resolving the
//! per-instance data slot. `themes` stays identity-free (app-owned
//! host palette — the `Theme` class has no field).
//!
//! The rpc bind-table rows (`rpc::bind` / `rpc::handler` — the phase-3
//! dispatch redesign) live HERE: their bodies relocate from
//! `ease_tur_rpc::install_rows` (the one-owner rule — emp_host is the
//! single mount), their identity decode is `ease_tur_rpc`'s
//! [`RpcIdentity`] stamp (the same minted id the `RpcClient` dispatches
//! with), and the view→backend `rpc::call` row keeps the envelope shape
//! behind the handle (phase 5 deleted the old stash-bus rows — this is
//! the only rpc surface).

use std::sync::Arc;

use ease_client_backend::repositories::core::DatabaseServer;
use rut_core::types::{TY_BOOL, TY_I64, TY_NIL, TY_OPAQUE, TY_OPT_STR, TY_STR, TY_U64};
use tur_engine::core::rut_runtime::{host_row_at, RutHandles};

use crate::plugin_runtime::backend_cx;

use super::super::service_rows::{
    op_context_create_storage, op_context_notify_change, op_context_remove_storage,
    op_db_list_keys, op_db_multi_append, op_db_multi_append_multi, op_db_multi_count,
    op_db_multi_count_multi, op_db_multi_delete, op_db_multi_delete_multi, op_db_multi_get_all,
    op_db_multi_get_all_multi, op_db_single_delete, op_db_single_delete_multi, op_db_single_get,
    op_db_single_get_multi, op_db_single_set, op_db_single_set_multi, op_library_covers,
    op_library_playlists, op_oauth_begin, op_oauth_new, op_secret_get, op_secret_put,
    op_secret_remove, op_theme_color, op_theme_is_dark,
};
use super::decode_handle;

/// The decl rows — the `emp_host` surface (paths `db` / `secret` /
/// `oauth` / `themes` / `context` / `library`; the tree mounts from
/// these). Every scoped row's FIRST parameter is the handle.
pub(crate) fn decl() -> Vec<rut_driver::HostRow> {
    let row = |path: &str, n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        host_row_at(path, n, p, r, false)
    };
    let h = |mut p: Vec<rut_core::types::TypeId>| {
        let mut out = vec![TY_OPAQUE];
        out.append(&mut p);
        out
    };
    vec![
        // ---- db (plugin-scoped KV). Composites ride JSON strings. ----
        row("db", "single_get", h(vec![TY_STR]), TY_OPT_STR),
        row("db", "single_get_multi", h(vec![TY_STR]), TY_STR),
        row("db", "single_set", h(vec![TY_STR, TY_STR]), TY_NIL),
        row("db", "single_set_multi", h(vec![TY_STR]), TY_NIL),
        row("db", "single_delete", h(vec![TY_STR]), TY_NIL),
        row("db", "single_delete_multi", h(vec![TY_STR]), TY_NIL),
        row("db", "multi_append", h(vec![TY_STR, TY_STR]), TY_NIL),
        row("db", "multi_append_multi", h(vec![TY_STR]), TY_NIL),
        row("db", "multi_get_all", h(vec![TY_STR]), TY_STR),
        row("db", "multi_get_all_multi", h(vec![TY_STR]), TY_STR),
        row("db", "multi_count", h(vec![TY_STR]), TY_U64),
        row("db", "multi_count_multi", h(vec![TY_STR]), TY_STR),
        row("db", "multi_delete", h(vec![TY_STR]), TY_NIL),
        row("db", "multi_delete_multi", h(vec![TY_STR]), TY_NIL),
        row("db", "list_keys", h(vec![TY_STR]), TY_STR),
        // ---- secret (plugin-scoped secret store) ----
        row("secret", "get", h(vec![TY_I64]), TY_OPT_STR),
        row("secret", "put", h(vec![TY_STR]), TY_I64),
        row("secret", "remove", h(vec![TY_I64]), TY_NIL),
        // ---- oauth (flow mint + Kotlin upcall) ----
        row("oauth", "new", h(vec![]), TY_STR),
        row("oauth", "begin", h(vec![TY_STR]), TY_NIL),
        // ---- themes (host palette — identity-free) ----
        row("themes", "color", vec![TY_STR], TY_STR),
        row("themes", "is_dark", vec![], TY_BOOL),
        // ---- context (storage lifecycle) ----
        row("context", "notify_change", h(vec![]), TY_NIL),
        row("context", "create_storage", h(vec![TY_STR]), TY_NIL),
        row("context", "remove_storage", h(vec![TY_STR]), TY_NIL),
        row("context", "storage_id", h(vec![]), TY_OPT_STR),
        // ---- library (read-only host data) ----
        row("library", "playlists", h(vec![]), TY_STR),
        row("library", "covers", h(vec![TY_STR]), TY_STR),
    ]
    // ---- rpc (the phase-3 bind table — bodies relocate from
    // ease-tur-rpc; the view→backend call row joins them here) ----
    .into_iter()
    .chain(ease_tur_rpc::bind_rows_decl())
    .chain(vec![row("rpc", "call", h(vec![TY_STR, TY_STR]), TY_STR)])
    .collect::<Vec<_>>()
}

/// The body pkg. Keys are the scope-relative path spellings
/// (`db::single_get`); the pkg scope (`emp_host`) prefixes at the
/// install — matching [`decl()`]'s `emp_host::db::single_get` full
/// names exactly (the boot verifies the join).
pub(crate) fn pkg(handles: &std::rc::Rc<RutHandles>) -> rut_vm::interp::HostPkg {
    let mut pkg = rut_vm::interp::HostPkg::new("emp_host");

    /// One scoped row: the leading handle opaque decodes against the
    /// calling instance's own stamp, then the shared op runs. The
    /// closure owns its `Rc<RutHandles>` clone (the
    /// `service_rows::install` discipline); `$handle` is spliced so
    /// the body sees the decoded handle under its own name.
    macro_rules! emp_row {
        ($handles:ident, $key:literal, ($($arg:ident: $ty:ty),*) -> $ret:ty, $ns:literal, $handle:ident, $body:block) => {{
            let h: std::rc::Rc<RutHandles> = $handles.clone();
            rut_vm::pkg_fn!(pkg, $key, (rut_vm::OpaqueRef, $($ty,)*) -> $ret, move |vm: &mut rut_vm::interp::Vm, hdl: rut_vm::OpaqueRef, $($arg: $ty),*| {
                // Rebind the call-site idents INSIDE the closure: `handles`
                // → the closure's own Rc clone, `$handle` → the decoded
                // plugin handle.
                let $handles = &h;
                let $handle = decode_handle(vm, &hdl, $ns, &h)?;
                #[allow(unused_variables)]
                let _ = vm;
                $body
            })
        }};
    }

    // ---- db ---------------------------------------------------------------

    emp_row!(handles, "db::single_get", (key: &str) -> Option<String>, "db", handle, {
        op_db_single_get(&db_of(handles)?, handle.plugin_id(), key)
    });

    emp_row!(handles, "db::single_get_multi", (keys_json: &str) -> String, "db", handle, {
        op_db_single_get_multi(&db_of(handles)?, handle.plugin_id(), keys_json)
    });

    emp_row!(handles, "db::single_set", (key: &str, value: &str) -> (), "db", handle, {
        op_db_single_set(&db_of(handles)?, handle.plugin_id(), key, value)
    });

    emp_row!(handles, "db::single_set_multi", (entries_json: &str) -> (), "db", handle, {
        op_db_single_set_multi(&db_of(handles)?, handle.plugin_id(), entries_json)
    });

    emp_row!(handles, "db::single_delete", (key: &str) -> (), "db", handle, {
        op_db_single_delete(&db_of(handles)?, handle.plugin_id(), key)
    });

    emp_row!(handles, "db::single_delete_multi", (keys_json: &str) -> (), "db", handle, {
        op_db_single_delete_multi(&db_of(handles)?, handle.plugin_id(), keys_json)
    });

    emp_row!(handles, "db::multi_append", (key: &str, value: &str) -> (), "db", handle, {
        op_db_multi_append(&db_of(handles)?, handle.plugin_id(), key, value)
    });

    emp_row!(handles, "db::multi_append_multi", (entries_json: &str) -> (), "db", handle, {
        op_db_multi_append_multi(&db_of(handles)?, handle.plugin_id(), entries_json)
    });

    emp_row!(handles, "db::multi_get_all", (key: &str) -> String, "db", handle, {
        op_db_multi_get_all(&db_of(handles)?, handle.plugin_id(), key)
    });

    emp_row!(handles, "db::multi_get_all_multi", (keys_json: &str) -> String, "db", handle, {
        op_db_multi_get_all_multi(&db_of(handles)?, handle.plugin_id(), keys_json)
    });

    emp_row!(handles, "db::multi_count", (key: &str) -> u64, "db", handle, {
        op_db_multi_count(&db_of(handles)?, handle.plugin_id(), key)
    });

    emp_row!(handles, "db::multi_count_multi", (keys_json: &str) -> String, "db", handle, {
        op_db_multi_count_multi(&db_of(handles)?, handle.plugin_id(), keys_json)
    });

    emp_row!(handles, "db::multi_delete", (key: &str) -> (), "db", handle, {
        op_db_multi_delete(&db_of(handles)?, handle.plugin_id(), key)
    });

    emp_row!(handles, "db::multi_delete_multi", (keys_json: &str) -> (), "db", handle, {
        op_db_multi_delete_multi(&db_of(handles)?, handle.plugin_id(), keys_json)
    });

    emp_row!(handles, "db::list_keys", (prefix: &str) -> String, "db", handle, {
        op_db_list_keys(&db_of(handles)?, handle.plugin_id(), prefix)
    });

    // ---- secret -------------------------------------------------------------

    emp_row!(handles, "secret::get", (id: i64) -> Option<String>, "secret", handle, {
        op_secret_get(&db_of(handles)?, handle.plugin_id(), id)
    });

    emp_row!(handles, "secret::put", (secret: &str) -> i64, "secret", handle, {
        op_secret_put(&db_of(handles)?, handle.plugin_id(), secret)
    });

    emp_row!(handles, "secret::remove", (id: i64) -> (), "secret", handle, {
        op_secret_remove(&db_of(handles)?, handle.plugin_id(), id)
    });

    // ---- oauth ---------------------------------------------------------------

    emp_row!(handles, "oauth::new", () -> String, "oauth", handle, {
        let _ = handle;
        let _ = handles;
        Ok(op_oauth_new())
    });

    emp_row!(handles, "oauth::begin", (oauth_id: &str) -> (), "oauth", handle, {
        let _ = handles;
        op_oauth_begin(handle.plugin_id(), oauth_id)
    });

    // ---- themes (identity-free: no leading handle) -----------------------------

    rut_vm::pkg_fn!(pkg, "themes::color", (&str,) -> String, move |_vm: &mut rut_vm::interp::Vm, name: &str| {
        op_theme_color(name)
    });

    rut_vm::pkg_fn!(pkg, "themes::is_dark", () -> bool, move |_vm: &mut rut_vm::interp::Vm| {
        op_theme_is_dark()
    });

    // ---- context --------------------------------------------------------------

    emp_row!(handles, "context::notify_change", () -> (), "context", handle, {
        let _ = handles;
        op_context_notify_change(handle.plugin_id(), handle.storage_instance().map(str::to_string));
        Ok(())
    });

    emp_row!(handles, "context::create_storage", (plugin_storage_id: &str) -> (), "context", handle, {
        op_context_create_storage(backend_cx("context", handles)?, handle.plugin_id(), plugin_storage_id);
        Ok(())
    });

    emp_row!(handles, "context::remove_storage", (plugin_storage_id: &str) -> (), "context", handle, {
        op_context_remove_storage(backend_cx("context", handles)?, handle.plugin_id(), plugin_storage_id);
        Ok(())
    });

    emp_row!(handles, "context::storage_id", () -> Option<String>, "context", handle, {
        let _ = handles;
        Ok(handle.storage_instance().map(str::to_string))
    });

    // ---- library ---------------------------------------------------------------

    emp_row!(handles, "library::playlists", () -> String, "library", handle, {
        let _ = handle;
        op_library_playlists(&backend_cx("library", handles)?)
    });

    emp_row!(handles, "library::covers", (music_ids_json: &str) -> String, "library", handle, {
        let _ = handle;
        op_library_covers(&backend_cx("library", handles)?, handles, music_ids_json)
    });

    // ---- rpc (the phase-3 bind table) -------------------------------------

    // The bind/dispatch rows' bodies mount from ease-tur-rpc (one owner —
    // the relocation seam); their leading-handle decode is the
    // `RpcIdentity` stamp (the same minted id `RpcClient` dispatches with).
    ease_tur_rpc::install_rows(&mut pkg, handles);

    // call(op, argsJson) -> envelope str — the view→backend call row, the
    // old `ease.rpc.call` shape behind the handle.
    emp_row!(handles, "rpc::call", (op: &str, args_json: &str) -> String, "rpc", handle, {
        let _ = handles;
        crate::plugin_runtime::service_rows::rpc_call_envelope(
            &backend_cx("rpc", handles)?,
            handle.plugin_id(),
            op,
            args_json,
        )
    });

    pkg
}

/// The row's database server (the backend context still resolves from
/// the per-instance plugin state — the `PluginBackendCx` stamp; the
/// handle carries only the plugin identity).
fn db_of(handles: &std::rc::Rc<RutHandles>) -> Result<Arc<DatabaseServer>, rut_vm::Trap> {
    Ok(backend_cx("db", handles)?.database_server().clone())
}
