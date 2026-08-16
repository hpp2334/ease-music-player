//! `ease.rpc` JS bridge — lets a view instance call a handler on its own
//! plugin's headless backend.
//!
//! `call(op, args): Promise<any>` resolves the calling plugin's identity
//! from the per-instance data slot, looks up the backend's wired `RpcClient`
//! (`BackendContext::service_rpc_for(pluginId)`), and awaits
//! `rpc.call(op, args)`. The promise settles with the handler's result, or
//! rejects with its error. This is the channel a tur-rendered view uses to
//! reach plugin-owned domain logic that lives in the backend instance
//! (e.g. `onedrive:removeInstance`).
//!
//! Implementation mirrors `tur:net`'s `request` bridge: mint a pending
//! `JsPromise`, spawn the async `rpc.call` via the instance's
//! `spawn_local`, and push a completion that resolves/rejects the promise
//! under `&mut Context` on the next flush.
//!
//! Bridge fns are ctx-bound (`FnEntry`): `bound_native` prepends the
//! per-instance `TurInstanceContext`, so the fn receives it as `args[0]`
//! and recovers it via `extract_js_ctx`.

use boa_engine::object::JsObject;
use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsResult, JsValue, JsVariant};
use tur_engine::core::js_runtime::helpers::{extract_js_ctx, FnEntry, Ptr};

use crate::plugin_runtime::PluginId;

/// Build the `FnEntry` table for the `rpc` namespace object.
pub fn build_fns() -> Vec<FnEntry> {
    vec![("call", 2, call as Ptr)]
}

/// `call(op, args): Promise<any>` — invoke handler `op` on this plugin's
/// backend with JSON-serializable `args`.
///
/// `args[0]` is the bound ctx value (prepended by `bound_native`); the
/// user's `op` is at index 1 and `args` at index 2. `args` may be omitted
/// (treated as `null`).
fn call(_this: &JsValue, args: &[JsValue], ctx: &mut boa_engine::Context) -> JsResult<JsValue> {
    use boa_engine::object::builtins::JsPromise;

    let js_ctx = extract_js_ctx(args)?;
    let pid = js_ctx.data::<PluginId>().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "ease:rpc: no plugin context bound to this instance",
        ))
    })?;

    let op = args
        .get_or_undefined(1)
        .as_string()
        .ok_or_else(|| {
            JsError::from(JsNativeError::typ()
                .with_message("ease:rpc.call: op (arg 0) must be a string"))
        })?
        .to_std_string_escaped();

    let args_val = args.get_or_undefined(2);
    let args_json = match args_val.variant() {
        JsVariant::Undefined | JsVariant::Null => serde_json::Value::Null,
        _ => args_val.to_json(ctx)?.unwrap_or(serde_json::Value::Null),
    };

    let Some(cx) = crate::BACKEND_CONTEXT.get() else {
        return Err(JsError::from(JsNativeError::typ().with_message(
            "ease:rpc.call: BACKEND_CONTEXT not set",
        )));
    };
    let Some(rpc) = cx.service_rpc_for(pid.as_ref()) else {
        return Err(JsError::from(JsNativeError::typ().with_message(format!(
            "ease:rpc.call: no service RPC wired for plugin {} (backend not up)",
            pid.as_str()
        ))));
    };

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    let completion_handle = js_ctx.completion_handle();

    let _ = js_ctx.spawn_local(move |_aw| async move {
        let result = rpc.call(&op, args_json).await;
        completion_handle.push(Box::new(move |ctx| {
            match result {
                Ok(value) => {
                    let js_val =
                        JsValue::from_json(&value, ctx).unwrap_or(JsValue::null());
                    let _ = resolvers
                        .resolve
                        .call(&JsValue::undefined(), &[js_val], ctx);
                }
                Err(e) => {
                    let err_obj = JsObject::with_object_proto(ctx.intrinsics());
                    let _ = err_obj.create_data_property(
                        js_string!("message"),
                        JsValue::from(js_string!(e.to_string().as_str())),
                        ctx,
                    );
                    let _ = resolvers
                        .reject
                        .call(&JsValue::undefined(), &[err_obj.into()], ctx);
                }
            }
            Ok(())
        }));
    });

    Ok(promise.into())
}
