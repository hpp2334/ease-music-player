//! JNI entrypoint for the bridge.
//!
//! Exposes a single symbol consumed by the Kotlin `EaseBridge` singleton:
//!
//! ```kotlin
//! object EaseBridge {
//!     init { /* System.loadLibrary done in EaseMusicPlayerApplication */ }
//!     external fun call(payloadJson: String, buffers: Array<ByteArray>?): BridgeResult
//! }
//! class BridgeResult(val payloadJson: String, val buffers: Array<ByteArray>?)
//! ```
//!
//! The Kotlin side lives in package `com.kutedev.easemusicplayer.singleton`,
//! so the symbol is `Java_com_kutedev_easemusicplayer_singleton_EaseBridge_call`.

use jni::{
    objects::{JByteArray, JClass, JObject, JObjectArray, JString, JValue},
    sys::jsize,
    JNIEnv,
};

use crate::bridge::{dispatch::dispatch, request::BridgeRequest};

const BRIDGE_RESULT_CLASS: &str = "com/kutedev/easemusicplayer/singleton/NativeBridgeResult";

/// Top-level JNI entrypoint.
///
/// Returns a `BridgeResult` POJO. On any JNI-level failure (class lookup,
/// array construction, request parse) the returned POJO carries an error
/// envelope in `payloadJson` and an empty buffers array.
#[no_mangle]
pub extern "system" fn Java_com_kutedev_easemusicplayer_singleton_EaseBridge_call<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    payload: JString<'local>,
    buffers: JObject<'local>,
) -> JObject<'local> {
    match inner(&mut env, payload, buffers) {
        Ok(obj) => obj,
        Err(e) => build_error_result(&mut env, &e).unwrap_or_else(|_| JObject::null()),
    }
}

fn inner<'local>(
    env: &mut JNIEnv<'local>,
    payload: JString<'local>,
    buffers: JObject<'local>,
) -> Result<JObject<'local>, String> {
    let payload_str: String = env
        .get_string(&payload)
        .map_err(|e| format!("get_string: {e:?}"))?
        .into();

    let req: BridgeRequest =
        serde_json::from_str(&payload_str).map_err(|e| format!("parse request: {e}"))?;

    // Collect input buffers (Array<ByteArray>? → Vec<Vec<u8>>). Kotlin
    // may pass null when the call carries no buffers; treat as empty.
    let input_bufs = if buffers.is_null() {
        Vec::new()
    } else {
        let buffers_arr: JObjectArray = buffers.into();
        collect_input_buffers(env, &buffers_arr)?
    };

    // Dispatch on the shared tokio runtime. This blocks the calling
    // (JNI) thread until the (possibly async) controller completes —
    // exactly the same pattern the existing `cts_*` functions use.
    let (resp_value, output_bufs): (serde_json::Value, Vec<Vec<u8>>) =
        ease_client_tokio::tokio_runtime().block_on(async { dispatch(req, input_bufs).await });

    // Serialize response to a JSON string. `dispatch` always returns a
    // well-formed envelope so this shouldn't fail; if it somehow does,
    // fall back to a generic error envelope.
    let payload_json = serde_json::to_string(&resp_value).unwrap_or_else(|_| {
        r#"{"success":false,"errorCode":"SerializationError","errorDetail":"to_string failed"}"#
            .to_string()
    });

    build_result_pojo(env, &payload_json, &output_bufs)
}

fn collect_input_buffers<'local>(
    env: &mut JNIEnv<'local>,
    buffers: &JObjectArray<'local>,
) -> Result<Vec<Vec<u8>>, String> {
    let len = env
        .get_array_length(buffers)
        .map_err(|e| format!("get_array_length: {e:?}"))?;
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
        let element = env
            .get_object_array_element(buffers, i)
            .map_err(|e| format!("get_object_array_element[{i}]: {e:?}"))?;
        let byte_array: JByteArray = element.into();
        let bytes = env
            .convert_byte_array(&byte_array)
            .map_err(|e| format!("convert_byte_array[{i}]: {e:?}"))?;
        out.push(bytes);
    }
    Ok(out)
}

fn build_result_pojo<'local>(
    env: &mut JNIEnv<'local>,
    payload_json: &str,
    buffers: &[Vec<u8>],
) -> Result<JObject<'local>, String> {
    let class = env
        .find_class(BRIDGE_RESULT_CLASS)
        .map_err(|e| format!("find_class {BRIDGE_RESULT_CLASS}: {e:?}"))?;
    // BridgeResult(String payloadJson, byte[][] buffers)
    // → JVM signature: (Ljava/lang/String;[[B)V
    let ctor_sig = "(Ljava/lang/String;[[B)V";

    let payload_jstr = env
        .new_string(payload_json)
        .map_err(|e| format!("new_string: {e:?}"))?;

    let buffers_arr = build_byte_array_array(env, buffers)?;

    let obj = env
        .new_object(
            &class,
            ctor_sig,
            &[JValue::from(&payload_jstr), JValue::from(&buffers_arr)],
        )
        .map_err(|e| format!("new_object: {e:?}"))?;

    Ok(obj)
}

fn build_byte_array_array<'local>(
    env: &mut JNIEnv<'local>,
    buffers: &[Vec<u8>],
) -> Result<JObjectArray<'local>, String> {
    // The element class for byte[] is described by the descriptor "[B".
    let element_class = env
        .find_class("[B")
        .map_err(|e| format!("find_class [B: {e:?}"))?;
    let arr = env
        .new_object_array(buffers.len() as jsize, &element_class, JObject::null())
        .map_err(|e| format!("new_object_array: {e:?}"))?;

    for (i, buf) in buffers.iter().enumerate() {
        let jbuf = env
            .byte_array_from_slice(buf)
            .map_err(|e| format!("byte_array_from_slice[{i}]: {e:?}"))?;
        env.set_object_array_element(&arr, i as jsize, &jbuf)
            .map_err(|e| format!("set_object_array_element[{i}]: {e:?}"))?;
    }

    Ok(arr)
}

fn build_error_result<'local>(
    env: &mut JNIEnv<'local>,
    message: &str,
) -> Result<JObject<'local>, ()> {
    let err = serde_json::json!({
        "success": false,
        "errorCode": "BridgeInternalError",
        "errorDetail": message,
    });
    let payload = serde_json::to_string(&err)
        .unwrap_or_else(|_| r#"{"success":false,"errorCode":"BridgeInternalError"}"#.to_string());
    build_result_pojo(env, &payload, &[]).map_err(|_| ())
}
