//! `ease:secret` JS bridge — owner-scoped secret access for plugins.
//!
//! Mirrors `ease:storage`: a ctx-free host module whose fns reach the backend
//! through [`crate::BACKEND_CONTEXT`], each call a short `block_on` on the
//! shared tokio runtime. The caller passes its own `pluginId`; the
//! [`SecretStore`] enforces that the row's `scope` is `"plugin:<pluginId>"` —
//! so a plugin can only read / write / remove secrets it owns. (Spoofing
//! another plugin's id is a residual risk that only matters once untrusted
//! third-party plugins exist; first-party plugins are trusted. Per-instance
//! binding will harden this later.)

use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction};
use ease_client_schema::{PluginId, SecretId, SecretScope};

use crate::error::BResult;
use crate::repositories::secret::SecretStore;

/// Build the export table for `ease:secret`. Returns
/// `(name, NativeFunction, length)` tuples for `register_host_module`.
pub fn build_host_fns() -> Vec<(&'static str, NativeFunction, usize)> {
    vec![
        ("get", NativeFunction::from_copy_closure(secret_get), 2),
        ("put", NativeFunction::from_copy_closure(secret_put), 2),
        ("remove", NativeFunction::from_copy_closure(secret_remove), 2),
    ]
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn require_string(args: &[JsValue], idx: usize) -> JsResult<String> {
    let v = args.get_or_undefined(idx);
    if v.is_undefined() || v.is_null() {
        return Err(JsError::from(JsNativeError::typ().with_message(format!(
            "ease:secret: missing required string argument at index {idx}"
        ))));
    }
    let s = v.as_string().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(format!(
            "ease:secret: expected string at index {idx}"
        )))
    })?;
    Ok(s.to_std_string_escaped())
}

fn require_i64(args: &[JsValue], idx: usize) -> JsResult<i64> {
    let v = args.get_or_undefined(idx);
    let n = v.as_number().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(format!(
            "ease:secret: expected number at index {idx}"
        )))
    })?;
    Ok(n as i64)
}

fn db_clone() -> JsResult<std::sync::Arc<crate::repositories::core::DatabaseServer>> {
    let cx = crate::BACKEND_CONTEXT.get().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "ease:secret: backend not initialized (BACKEND_CONTEXT is unset)",
        ))
    })?;
    Ok(cx.database_server().clone())
}

fn plugin_scope(plugin_id: String) -> SecretScope {
    SecretScope::Plugin(PluginId::new(plugin_id))
}

fn unwrap_blocking<R>(description: &str, result: BResult<R>) -> JsResult<R> {
    result.map_err(|e| {
        JsError::from(JsNativeError::typ().with_message(format!(
            "ease:secret {description} failed: {e:?}"
        )))
    })
}

// ---------------------------------------------------------------------------
// bridge fns
// ---------------------------------------------------------------------------

/// `get(pluginId, secretId) -> string | null`. Returns `null` if the secret
/// does not exist OR is not owned by `pluginId` (no existence leak).
fn secret_get(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let plugin_id = require_string(args, 0)?;
    let id = require_i64(args, 1)?;
    let db = db_clone()?;
    let scope = plugin_scope(plugin_id);
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.secret_get(scope, SecretId::wrap(id)).await
    });
    match unwrap_blocking("get", result)? {
        Some(value) => Ok(JsValue::from(js_string!(value.as_str()))),
        None => Ok(JsValue::null()),
    }
}

/// `put(pluginId, secret) -> secretId`. Stores a new secret owned by
/// `pluginId` and returns its id.
fn secret_put(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let plugin_id = require_string(args, 0)?;
    let secret = require_string(args, 1)?;
    let db = db_clone()?;
    let scope = plugin_scope(plugin_id);
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.secret_put(scope, secret).await
    });
    let id = unwrap_blocking("put", result)?;
    Ok(JsValue::from(*id.as_ref() as f64))
}

/// `remove(pluginId, secretId) -> undefined`. No-op if the secret does not
/// exist or is not owned by `pluginId`.
fn secret_remove(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, 0)?;
    let id = require_i64(args, 1)?;
    let db = db_clone()?;
    let scope = plugin_scope(plugin_id);
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.secret_remove(scope, SecretId::wrap(id)).await
    });
    unwrap_blocking("remove", result)?;
    Ok(JsValue::undefined())
}
