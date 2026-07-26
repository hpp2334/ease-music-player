//! `ease:storage` JS bridge — synchronous KV access for plugins.
//!
//! All operations run via `tokio_runtime().block_on(...)` because tur's
//! boa engine is single-threaded and `!Send`. Each call blocks the
//! engine thread for the SQLite round-trip (~ms-scale). Multi-key
//! variants use indexed `IN (...)` queries so the per-call cost is
//! independent of the key count.
//!
//! The bridge is registered as a **host module** (ctx-free) because
//! none of the fns need tur's `TurJsContext` — they all reach the
//! backend via the [`crate::BACKEND_CONTEXT`] OnceLock.

use boa_engine::object::builtins::JsArray;
use boa_engine::object::JsObject;
use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsResult, JsValue, NativeFunction};
use ease_client_schema::PluginKvEntry;

use crate::error::BResult;

const ARG_PLUGIN_ID: usize = 0;

/// Build the export table for `ease:storage`. Returns
/// `(name, NativeFunction, length)` tuples for `register_host_module`.
pub fn build_host_fns() -> Vec<(&'static str, NativeFunction, usize)> {
    vec![
        // ---- single-value (overwrite) ----
        ("singleGet", NativeFunction::from_copy_closure(single_get), 2),
        ("singleGetMulti", NativeFunction::from_copy_closure(single_get_multi), 2),
        ("singleSet", NativeFunction::from_copy_closure(single_set), 3),
        ("singleSetMulti", NativeFunction::from_copy_closure(single_set_multi), 2),
        ("singleDelete", NativeFunction::from_copy_closure(single_delete), 2),
        ("singleDeleteMulti", NativeFunction::from_copy_closure(single_delete_multi), 2),
        // ---- multi-value (append) ----
        ("multiAppend", NativeFunction::from_copy_closure(multi_append), 3),
        ("multiAppendMulti", NativeFunction::from_copy_closure(multi_append_multi), 2),
        ("multiGetAll", NativeFunction::from_copy_closure(multi_get_all), 2),
        ("multiGetAllMulti", NativeFunction::from_copy_closure(multi_get_all_multi), 2),
        ("multiCount", NativeFunction::from_copy_closure(multi_count), 2),
        ("multiCountMulti", NativeFunction::from_copy_closure(multi_count_multi), 2),
        ("multiDelete", NativeFunction::from_copy_closure(multi_delete), 2),
        ("multiDeleteMulti", NativeFunction::from_copy_closure(multi_delete_multi), 2),
        // ---- listing ----
        ("listKeys", NativeFunction::from_copy_closure(list_keys), 2),
    ]
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn require_string(args: &[JsValue], idx: usize) -> JsResult<String> {
    let v = args.get_or_undefined(idx);
    if v.is_undefined() || v.is_null() {
        return Err(JsError::from(JsNativeError::typ().with_message(format!(
            "ease:storage: missing required string argument at index {idx}"
        ))));
    }
    let s = v.as_string().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(format!(
            "ease:storage: expected string at index {idx}"
        )))
    })?;
    Ok(s.to_std_string_escaped())
}

fn read_string_array(arg: &JsValue, ctx: &mut boa_engine::Context) -> JsResult<Vec<String>> {
    let obj = arg.as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("ease:storage: expected array"))
    })?;
    let arr = JsArray::from_object(obj.clone()).map_err(|_| {
        JsError::from(JsNativeError::typ().with_message("ease:storage: expected array"))
    })?;
    let len = arr.length(ctx).unwrap_or(0);
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
        let v = arr.at(i as i64, ctx)?;
        let s = v.as_string().ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message(
                "ease:storage: array elements must be strings",
            ))
        })?;
        out.push(s.to_std_string_escaped());
    }
    Ok(out)
}

fn read_entry_array(
    arg: &JsValue,
    ctx: &mut boa_engine::Context,
) -> JsResult<Vec<PluginKvEntry>> {
    let obj = arg.as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("ease:storage: expected array"))
    })?;
    let arr = JsArray::from_object(obj.clone()).map_err(|_| {
        JsError::from(JsNativeError::typ().with_message("ease:storage: expected array"))
    })?;
    let len = arr.length(ctx).unwrap_or(0);
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
        let v = arr.at(i as i64, ctx)?;
        let obj = v.as_object().ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message(
                "ease:storage: array elements must be { key, value }",
            ))
        })?;
        let key = obj
            .get(js_string!("key"), ctx)?
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message(
                    "ease:storage: entry.key must be a string",
                ))
            })?
            .to_std_string_escaped();
        let value = obj
            .get(js_string!("value"), ctx)?
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message(
                    "ease:storage: entry.value must be a string",
                ))
            })?
            .to_std_string_escaped();
        out.push(PluginKvEntry { key, value });
    }
    Ok(out)
}

fn make_entry_object(key: &str, value: &str, ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let o = JsObject::with_object_proto(ctx.intrinsics());
    o.create_data_property(js_string!("key"), JsValue::from(js_string!(key)), ctx)?;
    o.create_data_property(js_string!("value"), JsValue::from(js_string!(value)), ctx)?;
    Ok(o.into())
}

fn make_multi_entry_object(
    key: &str,
    values: &[String],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let o = JsObject::with_object_proto(ctx.intrinsics());
    o.create_data_property(js_string!("key"), JsValue::from(js_string!(key)), ctx)?;
    let arr = JsArray::new(ctx)?;
    for (i, v) in values.iter().enumerate() {
        arr.set(i as u32, JsValue::from(js_string!(v.as_str())), true, ctx)?;
    }
    let arr_value: JsValue = arr.into();
    o.create_data_property(js_string!("values"), arr_value, ctx)?;
    Ok(o.into())
}

fn make_count_object(key: &str, count: u64, ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let o = JsObject::with_object_proto(ctx.intrinsics());
    o.create_data_property(js_string!("key"), JsValue::from(js_string!(key)), ctx)?;
    o.create_data_property(js_string!("count"), JsValue::from(count as f64), ctx)?;
    Ok(o.into())
}

fn run_blocking<R>(description: &str, result: BResult<R>) -> JsResult<R> {
    result.map_err(|e| {
        JsError::from(JsNativeError::typ().with_message(format!(
            "ease:storage {description} failed: {e:?}"
        )))
    })
}

fn db_clone() -> JsResult<std::sync::Arc<crate::repositories::core::DatabaseServer>> {
    let cx = crate::BACKEND_CONTEXT.get().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "ease:storage: backend not initialized (BACKEND_CONTEXT is unset)",
        ))
    })?;
    Ok(cx.database_server().clone())
}

// ---------------------------------------------------------------------------
// single-value bridge fns
// ---------------------------------------------------------------------------

fn single_get(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let key = require_string(args, 1)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_single_get(&plugin_id, &key).await
    });
    match run_blocking("singleGet", result)? {
        Some(value) => Ok(JsValue::from(js_string!(value.as_str()))),
        None => Ok(JsValue::null()),
    }
}

fn single_get_multi(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let keys = read_string_array(args.get_or_undefined(1), ctx)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_single_get_multi(&plugin_id, keys).await
    });
    let entries = run_blocking("singleGetMulti", result)?;
    let arr = JsArray::new(ctx)?;
    for (i, e) in entries.iter().enumerate() {
        let o = make_entry_object(&e.key, &e.value, ctx)?;
        arr.set(i as u32, o, true, ctx)?;
    }
    Ok(arr.into())
}

fn single_set(_this: &JsValue, args: &[JsValue], _ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let key = require_string(args, 1)?;
    let value = require_string(args, 2)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_single_set(&plugin_id, &key, &value).await
    });
    run_blocking("singleSet", result)?;
    Ok(JsValue::undefined())
}

fn single_set_multi(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let entries = read_entry_array(args.get_or_undefined(1), ctx)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_single_set_multi(&plugin_id, entries).await
    });
    run_blocking("singleSetMulti", result)?;
    Ok(JsValue::undefined())
}

fn single_delete(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let key = require_string(args, 1)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_single_delete(&plugin_id, &key).await
    });
    run_blocking("singleDelete", result)?;
    Ok(JsValue::undefined())
}

fn single_delete_multi(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let keys = read_string_array(args.get_or_undefined(1), ctx)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_single_delete_multi(&plugin_id, keys).await
    });
    run_blocking("singleDeleteMulti", result)?;
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// multi-value (append) bridge fns
// ---------------------------------------------------------------------------

fn multi_append(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let key = require_string(args, 1)?;
    let value = require_string(args, 2)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_multi_append(&plugin_id, &key, &value).await
    });
    run_blocking("multiAppend", result)?;
    Ok(JsValue::undefined())
}

fn multi_append_multi(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let entries = read_entry_array(args.get_or_undefined(1), ctx)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_multi_append_multi(&plugin_id, entries).await
    });
    run_blocking("multiAppendMulti", result)?;
    Ok(JsValue::undefined())
}

fn multi_get_all(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let key = require_string(args, 1)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_multi_get_all(&plugin_id, &key).await
    });
    let values = run_blocking("multiGetAll", result)?;
    let arr = JsArray::new(ctx)?;
    for (i, v) in values.iter().enumerate() {
        arr.set(i as u32, JsValue::from(js_string!(v.as_str())), true, ctx)?;
    }
    Ok(arr.into())
}

fn multi_get_all_multi(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let keys = read_string_array(args.get_or_undefined(1), ctx)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_multi_get_all_multi(&plugin_id, keys).await
    });
    let entries = run_blocking("multiGetAllMulti", result)?;
    let arr = JsArray::new(ctx)?;
    for (i, e) in entries.iter().enumerate() {
        let o = make_multi_entry_object(&e.key, &e.values, ctx)?;
        arr.set(i as u32, o, true, ctx)?;
    }
    Ok(arr.into())
}

fn multi_count(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let key = require_string(args, 1)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_multi_count(&plugin_id, &key).await
    });
    let count = run_blocking("multiCount", result)?;
    Ok(JsValue::from(count as f64))
}

fn multi_count_multi(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let keys = read_string_array(args.get_or_undefined(1), ctx)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_multi_count_multi(&plugin_id, keys).await
    });
    let entries = run_blocking("multiCountMulti", result)?;
    let arr = JsArray::new(ctx)?;
    for (i, e) in entries.iter().enumerate() {
        let o = make_count_object(&e.key, e.count, ctx)?;
        arr.set(i as u32, o, true, ctx)?;
    }
    Ok(arr.into())
}

fn multi_delete(
    _this: &JsValue,
    args: &[JsValue],
    _ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let key = require_string(args, 1)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_multi_delete(&plugin_id, &key).await
    });
    run_blocking("multiDelete", result)?;
    Ok(JsValue::undefined())
}

fn multi_delete_multi(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let keys = read_string_array(args.get_or_undefined(1), ctx)?;
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_multi_delete_multi(&plugin_id, keys).await
    });
    run_blocking("multiDeleteMulti", result)?;
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// listing
// ---------------------------------------------------------------------------

fn list_keys(_this: &JsValue, args: &[JsValue], ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    let plugin_id = require_string(args, ARG_PLUGIN_ID)?;
    let prefix_v = args.get_or_undefined(1);
    let prefix = if prefix_v.is_undefined() || prefix_v.is_null() {
        String::new()
    } else {
        prefix_v
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message(
                    "ease:storage: prefix must be a string",
                ))
            })?
            .to_std_string_escaped()
    };
    let db = db_clone()?;
    let result = ease_client_tokio::tokio_runtime().block_on(async move {
        db.plugin_kv_list_keys(&plugin_id, &prefix).await
    });
    let entries = run_blocking("listKeys", result)?;
    let arr = JsArray::new(ctx)?;
    for (i, info) in entries.iter().enumerate() {
        let o = JsObject::with_object_proto(ctx.intrinsics());
        o.create_data_property(
            js_string!("key"),
            JsValue::from(js_string!(info.key.as_str())),
            ctx,
        )?;
        o.create_data_property(
            js_string!("kind"),
            JsValue::from(info.kind.as_i32() as f64),
            ctx,
        )?;
        let o_value: JsValue = o.into();
        arr.set(i as u32, o_value, true, ctx)?;
    }
    Ok(arr.into())
}
