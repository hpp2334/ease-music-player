package com.kutedev.easemusicplayer.singleton

import com.kutedev.easemusicplayer.singleton.types.BridgeResponse
import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull

/**
 * Outcome of a [Bridge.call] — a typed envelope around the raw
 * [BridgeResponse]. Holds the [serializer] needed to materialize the
 * payload as [T] when the caller asks for it.
 *
 * Two unwrap modes:
 *  - [unwrapOrNull] — swallows the error (logs via the supplied `logFn`)
 *    and returns `null`. Use for fire-and-forget calls where the caller
 *    can tolerate a silent failure.
 *  - [unwrapOrThrow] — propagates [BridgeException]. Use when the caller
 *    needs to surface the error to the user (e.g. WebDAV "test
 *    connection").
 *
 * The [T] parameter flows from the [BridgeSpec] so that [BridgeResult]
 * exposes a strongly-typed [payload] accessor — callers cannot
 * accidentally decode with the wrong serializer.
 */
class BridgeRet<T> internal constructor(
    private val resp: BridgeResponse,
    private val buffers: Array<ByteArray>?,
    private val json: Json,
    private val serializer: KSerializer<T>,
    private val methodName: String,
    private val logFn: (String) -> Unit,
) {
    /** Whether the call succeeded at the backend level. */
    val isSuccess: Boolean get() = resp.success

    /** Error code if [isSuccess] is false; null otherwise. */
    val errorCode: String? get() = resp.errorCode

    /** Error detail (any JSON shape) if [isSuccess] is false; null otherwise. */
    val errorDetail: JsonElement? get() = resp.errorDetail

    /**
     * Returns the typed success value, or `null` after logging the error
     * via `logFn`. Use when the caller tolerates silent failure.
     */
    fun unwrapOrNull(): BridgeResult<T>? {
        if (!resp.success) {
            logFn("bridge.$methodName failed: ${resp.errorCode} ${resp.errorDetail ?: ""}")
            return null
        }
        return BridgeResult(resp.payload, buffers, json, serializer)
    }

    /**
     * Returns the typed success value, or throws [BridgeException] on
     * error. Use when the caller needs to surface the error.
     */
    fun unwrapOrThrow(): BridgeResult<T> {
        if (resp.success) {
            return BridgeResult(resp.payload, buffers, json, serializer)
        }
        throw com.kutedev.easemusicplayer.singleton.types.BridgeException(
            resp.errorCode ?: "Unknown",
            resp.errorDetail,
        )
    }
}

/**
 * Unwrapped success value of a [BridgeRet]. Exposes the typed [payload]
 * (decoded via the spec's serializer) plus optional binary [getBuffer]
 * accessors for buffer-returning calls (e.g. `asset.get`).
 *
 * For Unit-returning methods, [payload] returns [Unit] without
 * deserializing — callers don't need to special-case this.
 */
class BridgeResult<T> internal constructor(
    private val rawPayload: JsonElement?,
    private val buffers: Array<ByteArray>?,
    private val json: Json,
    private val serializer: KSerializer<T>,
) {
    /**
     * Typed payload. Throws if the payload field is absent (only happens
     * if the Rust dispatcher is buggy — every method sets a payload, even
     * Unit-returning ones set `null`).
     *
     * For Unit-returning specs, returns [Unit] without decoding.
     *
     * For nullable-T specs (e.g. `T = Playlist?`), returns `null` if the
     * Rust side returned `Value::Null` (kotlinx's nullable serializer
     * handles JsonNull → null).
     */
    val payload: T
        get() {
            if (serializer.isUnitSerializer()) {
                @Suppress("UNCHECKED_CAST")
                return Unit as T
            }
            return json.decodeFromJsonElement(serializer, rawPayload ?: JsonNull)
        }

    /**
     * Like [payload] but returns `null` if the payload is missing or
     * `JsonNull`. Useful for callers that want to tolerate both
     * "value absent" and "value present but null" uniformly.
     */
    val payloadOrNull: T?
        get() {
            if (serializer.isUnitSerializer()) {
                @Suppress("UNCHECKED_CAST")
                return Unit as T
            }
            if (rawPayload == null || rawPayload is JsonNull) return null
            return json.decodeFromJsonElement(serializer, rawPayload)
        }

    /** Raw JSON of the payload field (may be `JsonNull`). Null if absent. */
    val rawPayloadJson: JsonElement? get() = rawPayload

    /** True iff [rawPayloadJson] is non-null and not [JsonNull]. */
    val hasPayload: Boolean get() = rawPayload != null && rawPayload !is JsonNull

    /** Fetches the buffer at [index] (referenced by `bytesIndex` in some payloads). */
    fun getBuffer(index: Int): ByteArray? = buffers?.getOrNull(index)

    /** Number of buffers attached to the response. */
    val bufferCount: Int get() = buffers?.size ?: 0
}
