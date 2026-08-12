//! `ease.secret` JS bridge — owner-scoped secret access for plugins.
//!
//! Bridge fns are **ctx-bound** (`FnEntry`): identity comes from the
//! per-instance data slot via `extract_js_ctx` + `js_ctx.data::<PluginId>()`,
//! never from a JS argument. The [`SecretStore`] enforces that the row's
//! `scope` is `"plugin:<pluginId>"` — a plugin can only read / write /
//! remove secrets it owns.

use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use ease_client_schema::{PluginId as SchemaPluginId, SecretId, SecretScope};
use tur_engine::core::js_runtime::helpers::{extract_js_ctx, FnEntry, Ptr};

use crate::error::BResult;
use crate::plugin_runtime::PluginId;
use crate::repositories::secret::SecretStore;

/// Build the `FnEntry` table for the `secret` namespace object.
pub fn build_fns() -> Vec<FnEntry> {
    vec![
        ("get", 1, secret_get as Ptr),
        ("put", 1, secret_put as Ptr),
        ("remove", 1, secret_remove as Ptr),
    ]
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn plugin_id(args: &[JsValue]) -> JsResult<PluginId> {
    let js_ctx = extract_js_ctx(args)?;
    js_ctx.data::<PluginId>().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "ease:secret: no plugin context bound to this instance",
        ))
    })
}

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

fn plugin_scope(pid: &PluginId) -> SecretScope {
    SecretScope::Plugin(SchemaPluginId::new(pid.as_str()))
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

/// `get(secretId) -> string | null`. Returns `null` if the secret does not
/// exist OR is not owned by the calling plugin (no existence leak).
fn secret_get(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let pid = plugin_id(args)?;
    let id = require_i64(args, 1)?;
    let db = db_clone()?;
    let scope = plugin_scope(&pid);
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.secret_get(scope, SecretId::wrap(id)).await
    });
    match unwrap_blocking("get", result)? {
        Some(value) => Ok(JsValue::from(js_string!(value.as_str()))),
        None => Ok(JsValue::null()),
    }
}

/// `put(secret) -> secretId`. Stores a new secret owned by the calling
/// plugin and returns its id.
fn secret_put(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let pid = plugin_id(args)?;
    let secret = require_string(args, 1)?;
    let db = db_clone()?;
    let scope = plugin_scope(&pid);
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.secret_put(scope, secret).await
    });
    let id = unwrap_blocking("put", result)?;
    Ok(JsValue::from(*id.as_ref() as f64))
}

/// `remove(secretId) -> undefined`. No-op if the secret does not exist or
/// is not owned by the calling plugin.
fn secret_remove(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let pid = plugin_id(args)?;
    let id = require_i64(args, 1)?;
    let db = db_clone()?;
    let scope = plugin_scope(&pid);
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.secret_remove(scope, SecretId::wrap(id)).await
    });
    unwrap_blocking("remove", result)?;
    Ok(JsValue::undefined())
}
