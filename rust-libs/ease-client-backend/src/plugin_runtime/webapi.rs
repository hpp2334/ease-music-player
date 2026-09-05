//! `crypto` Web Platform polyfill for the plugin JS runtime (boa).
//!
//! boa implements ECMAScript but none of the browser globals — npm packages
//! routinely assume `crypto` exists. Randomness cannot be polyfilled in JS
//! (short of `Math.random`, which defeats the point), so the host installs
//! spec-shaped implementations backed by OS entropy into every JS instance:
//!
//! - `crypto.getRandomValues(view)` — integer typed arrays only (floats are
//!   a `TypeError`), 65536-byte quota (`RangeError`), returns the same view.
//! - `crypto.randomUUID()` — RFC 4122 v4 from the same entropy source.
//!
//! This is what unblocks ordinary crypto-dependent packages (e.g. `uuid`)
//! in plugin bundles. The *text* codecs (`TextEncoder` / `TextDecoder`)
//! are polyfilled on the JS side instead — see
//! `plugins/infra/text-polyfill.ts`, which delegates to `tur:std`'s
//! `encodeUtf8` / `decodeUtf8` rather than duplicating UTF-8 here.
//!
//! Installed as plain **globals** (not `ease.*`) by
//! [`super::plugin::EaseMusicPlugin::register`], which re-runs per instance
//! — headless backends and `TurView` views both get them.

use boa_engine::builtins::typed_array::TypedArrayKind;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::{JsArrayBuffer, JsTypedArray};
use boa_engine::object::{FunctionObjectBuilder, JsObject};
use boa_engine::property::Attribute;
use boa_engine::{js_string, Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

type NativeFn = fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>;

/// Install the `crypto` global into `ctx`. Idempotent — re-installing
/// overwrites in place.
pub fn install_globals(ctx: &mut Context) {
    let crypto = JsObject::with_object_proto(ctx.intrinsics());
    let _ = crypto.create_data_property_or_throw(
        js_string!("getRandomValues"),
        JsValue::from(function_object(ctx, "getRandomValues", 1, get_random_values)),
        ctx,
    );
    let _ = crypto.create_data_property_or_throw(
        js_string!("randomUUID"),
        JsValue::from(function_object(ctx, "randomUUID", 0, random_uuid)),
        ctx,
    );
    let _ = ctx.register_global_property(js_string!("crypto"), crypto, Attribute::all());
}

fn function_object(ctx: &mut Context, name: &str, length: usize, f: NativeFn) -> JsObject {
    FunctionObjectBuilder::new(ctx.realm(), NativeFunction::from_fn_ptr(f))
        .name(js_string!(name))
        .length(length)
        .build()
        .into()
}

/// `crypto.getRandomValues(view)` — fill an integer typed array with OS
/// entropy and return the same view. Float arrays are a `TypeError`; views
/// over 65536 bytes are a `RangeError` (spec quota).
fn get_random_values(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let view = args.get_or_undefined(0).as_object().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ()
                .with_message("crypto.getRandomValues: expected a typed array view"),
        )
    })?;
    let ta = JsTypedArray::from_object(view.clone())?;
    match ta.kind() {
        Some(TypedArrayKind::Float32) | Some(TypedArrayKind::Float64) => {
            return Err(JsError::from(
                JsNativeError::typ()
                    .with_message("crypto.getRandomValues: float arrays are not supported"),
            ));
        }
        _ => {}
    }
    let byte_length = ta.byte_length(ctx)?;
    if byte_length > 65536 {
        return Err(JsError::from(
            JsNativeError::range()
                .with_message("crypto.getRandomValues: byte length exceeds the 65536-byte quota"),
        ));
    }
    let byte_offset = ta.byte_offset(ctx)?;
    let buffer_val = ta.buffer(ctx)?;
    let buffer = JsArrayBuffer::from_object(
        buffer_val.as_object().ok_or_else(|| {
            JsError::from(
                JsNativeError::typ()
                    .with_message("crypto.getRandomValues: view has no backing buffer"),
            )
        })?,
    )?;
    let mut data = buffer.data_mut().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("crypto.getRandomValues: detached buffer"),
        )
    })?;
    let end = byte_offset + byte_length;
    getrandom::fill(&mut data[byte_offset..end]).map_err(|e| {
        JsError::from(
            JsNativeError::typ()
                .with_message(format!("crypto.getRandomValues: entropy source failed: {e}")),
        )
    })?;
    // Spec: return the same view that was passed in.
    Ok(JsValue::from(view))
}

/// `crypto.randomUUID()` — RFC 4122 v4 (lowercase hex, version/variant bits
/// set) from OS entropy.
fn random_uuid(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| {
        JsError::from(
            JsNativeError::typ()
                .with_message(format!("crypto.randomUUID: entropy source failed: {e}")),
        )
    })?;
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let s = format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    Ok(JsValue::from(js_string!(s.as_str())))
}

#[cfg(test)]
mod tests {
    use super::install_globals;
    use boa_engine::{Context, Source};

    fn setup() -> Context {
        let mut ctx = Context::default();
        install_globals(&mut ctx);
        ctx
    }

    fn eval_bool(ctx: &mut Context, code: &str) -> bool {
        ctx.eval(Source::from_bytes(code))
            .expect("eval ok")
            .as_boolean()
            .expect("boolean result")
    }

    fn eval_str(ctx: &mut Context, code: &str) -> String {
        ctx.eval(Source::from_bytes(code))
            .expect("eval ok")
            .as_string()
            .expect("string result")
            .to_std_string_escaped()
    }

    #[test]
    fn crypto_global_exists() {
        let mut ctx = setup();
        assert!(eval_bool(
            &mut ctx,
            "typeof crypto === 'object' && typeof crypto.getRandomValues === 'function' && typeof crypto.randomUUID === 'function'"
        ));
    }

    #[test]
    fn get_random_values_semantics() {
        let mut ctx = setup();
        // Same view returned; filled with entropy (all-zero is ~2^-128).
        assert!(eval_bool(
            &mut ctx,
            "const a = new Uint8Array(16); crypto.getRandomValues(a) === a && a.some(b => b !== 0)"
        ));
        // Works for non-byte integer views too.
        assert!(eval_bool(
            &mut ctx,
            "const w = new Uint16Array(4); crypto.getRandomValues(w); w.some(b => b !== 0)"
        ));
        // Float views are rejected; the 65536-byte quota is enforced.
        assert_eq!(
            eval_str(
                &mut ctx,
                "try { crypto.getRandomValues(new Float64Array(2)); 'no-throw'; } catch (e) { e.name; }"
            ),
            "TypeError"
        );
        assert_eq!(
            eval_str(
                &mut ctx,
                "try { crypto.getRandomValues(new Uint8Array(65537)); 'no-throw'; } catch (e) { e.name; }"
            ),
            "RangeError"
        );
        // The quota boundary itself is fine.
        assert!(eval_bool(
            &mut ctx,
            "crypto.getRandomValues(new Uint8Array(65536)) instanceof Uint8Array"
        ));
    }

    #[test]
    fn random_uuid_is_rfc4122_v4() {
        let mut ctx = setup();
        assert!(eval_bool(
            &mut ctx,
            "/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(crypto.randomUUID())"
        ));
    }
}
