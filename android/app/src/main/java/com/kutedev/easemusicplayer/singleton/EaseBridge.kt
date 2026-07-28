package com.kutedev.easemusicplayer.singleton

/**
 * Result of a [EaseBridge.call] invocation. Constructed on the native side
 * via JNI reflection (`NativeBridgeResult(String payloadJson, byte[][] buffers)`).
 *
 * - `payloadJson`: JSON envelope — either `{ "success": true, "payload": <T> }`
 *   or `{ "success": false, "errorCode": "...", "errorDetail": ... }`.
 * - `buffers`: optional `Array<ByteArray>` carrying binary payloads
 *   referenced by `bytesIndex: N` inside the JSON payload. Null when the
 *   call carries no bytes (the common case).
 *
 * Named `Native*` to avoid clash with the typed [BridgeResult] wrapper
 * that callers actually use.
 */
class NativeBridgeResult(
    val payloadJson: String,
    val buffers: Array<ByteArray>?,
)

/**
 * JNI surface to the Rust backend. One symbol,
 * [call], replaced the entire UniFFI-generated Kotlin bindings (~30 files).
 *
 * The native library is loaded once by `EaseMusicPlayerApplication` via
 * `System.loadLibrary("ease_client_backend")` — the same `.so` that hosts
 * the tur engine and the JNI entrypoint for `nativeInitAndroidContext`.
 *
 * Companion is declared `object` so the `external fun` resolves to a
 * static native method (Java_..._EaseBridge_call).
 */
object EaseBridge {
    external fun call(payloadJson: String, buffers: Array<ByteArray>?): NativeBridgeResult
}
