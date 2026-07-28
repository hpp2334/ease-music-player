package com.kutedev.easemusicplayer.singleton

import android.content.Context
import com.kutedev.easemusicplayer.singleton.types.ArgInitializeApp
import com.kutedev.easemusicplayer.singleton.types.BridgeResponse
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import javax.inject.Inject
import javax.inject.Singleton


private fun normalizePath(p: String): String =
    if (p.endsWith("/")) p else "$p/"


/**
 * Singleton wrapper around [EaseBridge] — the unified JSON+buffer bridge
 * to the Rust backend.
 *
 * Three long-lived handles are stored after [initialize]:
 *  - [backendIdValue] — the principal handle for data-layer methods.
 *  - [playerContextId] — for `player.contextNew` result.
 *  - [playerId] — for `player.new` result.
 *
 * ## API surface
 *
 * Two call paths:
 *
 * 1. **Typed catalog path** (preferred). Each method is declared in
 *    [BridgeMethods] as a [BridgeSpec] carrying the wire name + arg
 *    serializer + ret serializer + handle kind. Callers go through
 *    [call] / [call] and unwrap via [BridgeRet.unwrapOrNull] /
 *    [BridgeRet.unwrapOrThrow].
 *
 *    ```
 *    val playlist: Playlist? = bridge
 *        .call(BridgeMethods.Playlist.GET, id)
 *        .unwrapOrNull()?.payload
 *    ```
 *
 * 2. **Low-level escape hatch** ([callRaw]). For methods that don't fit
 *    the typed shape: `asset.get` (returns buffer), `player.loadMusic`
 *    (cross-handle args), `backend.*` lifecycle, `debug.trigger*`
 *    diagnostics. Returns [BridgeRet] of [JsonElement]; caller accesses
 *    [BridgeResult.rawPayloadJson] / [BridgeResult.getBuffer] directly.
 *
 *    ```
 *    val bytes: ByteArray? = bridge
 *        .callRaw("asset.get", args)
 *        .unwrapOrNull()?.getBuffer(0)
 *    ```
 */
@Singleton
class Bridge @Inject constructor(
    @ApplicationContext cx: Context,
) {
    @PublishedApi
    internal val json = Json {
        ignoreUnknownKeys = true
        // Encode all fields, even those equal to their default. Without
        // this, Rust structs whose fields have defaults on the Kotlin
        // side (e.g. ArgUpsertStorage.isAnonymous=false) would be
        // omitted from the wire payload, causing "missing field" errors
        // on the Rust deserializer.
        encodeDefaults = true
        // Discriminator for sealed classes — must match Rust's
        // `#[serde(tag = "kind")]` on enums that flow across the bridge
        // (ListStorageEntryChildrenResp, DataSourceKey). Default is
        // "type" which doesn't match Rust.
        classDiscriminator = "kind"
    }

    @PublishedApi internal var backendIdValue: Long = -1L
    private var playerContextId: Long = -1L
    private var playerId: Long = -1L
    private var _isInit = false

    private val arg = ArgInitializeApp(
        appDocumentDir = normalizePath(cx.filesDir.absolutePath),
        appCacheDir = normalizePath(cx.cacheDir.absolutePath),
        storagePath = "/",
    )

    // ------------------------------------------------------------------------
    // Typed catalog calls
    // ------------------------------------------------------------------------

    /** Arg-less typed call. Returns a [BridgeRet] you unwrap on the call site. */
    suspend fun <R> call(spec: BridgeSpec.NoArg<R>): BridgeRet<R> =
        invokeTyped(spec.name, buildJsonObject {}, null, spec.handleKind, spec.retSerializer)

    /** Typed call with one serializable arg. */
    suspend fun <A, R> call(spec: BridgeSpec.WithArg<A, R>, arg: A): BridgeRet<R> {
        val args = encodeArg(spec.argSerializer, arg)
        return invokeTyped(spec.name, args, null, spec.handleKind, spec.retSerializer)
    }
    // ------------------------------------------------------------------------
    // Low-level escape hatch — for buffer/cross-handle/lifecycle methods.
    // ------------------------------------------------------------------------

    /**
     * Low-level call. Constructs the request envelope manually and
     * returns a [BridgeRet] of [JsonElement]. Use only for methods that
     * don't fit the typed catalog (see [BridgeMethods] doc for the
     * exclusion list).
     */
    suspend fun callRaw(
        method: String,
        args: JsonElement = buildJsonObject {},
        buffers: Array<ByteArray>? = null,
        handle: Long = backendIdValue,
    ): BridgeRet<JsonElement> = withContext(Dispatchers.IO) {
        val req = buildJsonObject {
            put("method", method)
            put("args", args)
            put("handle", handle)
        }
        val native = EaseBridge.call(req.toString(), buffers)
        val resp = json.decodeFromString(BridgeResponse.serializer(), native.payloadJson)
        BridgeRet(
            resp = resp,
            buffers = native.buffers,
            json = json,
            serializer = JsonElement.serializer(),
            methodName = method,
            logFn = { msg -> logRaw("error", msg) },
        )
    }

    // ------------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------------

    fun initialize() {
        if (_isInit) return
        val req = buildJsonObject {
            put("method", "backend.create")
            put(
                "args",
                json.encodeToJsonElement(
                    ArgInitializeApp.serializer(),
                    arg,
                ),
            )
        }
        val resp = EaseBridge.call(req.toString(), null)
        val parsed = json.decodeFromString(BridgeResponse.serializer(), resp.payloadJson)
        if (!parsed.success) {
            throw RuntimeException("backend.create failed: ${parsed.errorCode} ${parsed.errorDetail}")
        }
        backendIdValue = (parsed.payload as JsonObject)["handle"]!!.jsonPrimitive.content.toLong()

        val initReq = buildJsonObject {
            put("method", "backend.init")
            put("handle", backendIdValue)
            put("args", buildJsonObject {})
        }
        val initResp = EaseBridge.call(initReq.toString(), null)
        val initParsed = json.decodeFromString(BridgeResponse.serializer(), initResp.payloadJson)
        if (!initParsed.success) {
            throw RuntimeException("backend.init failed: ${initParsed.errorCode} ${initParsed.errorDetail}")
        }
        logRaw("info", "bridge initialized (backendId=$backendIdValue)")
        _isInit = true
    }

    fun destroy() {
        if (!_isInit) return
        val req = buildJsonObject {
            put("method", "backend.deinit")
            put("handle", backendIdValue)
            put("args", buildJsonObject {})
        }
        EaseBridge.call(req.toString(), null)
        backendIdValue = -1L
        playerContextId = -1L
        playerId = -1L
        _isInit = false
        logRaw("info", "bridge destroyed")
    }

    // ------------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------------

    /** Routes a log message through the Rust tracing subscriber. */
    fun logRaw(level: String, message: String) {
        val req = buildJsonObject {
            put("method", "backend.log")
            put("args", buildJsonObject {
                put("level", level)
                put("message", message)
            })
        }
        try {
            EaseBridge.call(req.toString(), null)
        } catch (_: Throwable) {
            // Logging must never throw.
        }
    }

    /** Player handle setters — used by PlayerControllerRepository during setupCantodeEngine. */
    fun setPlayerContextId(id: Long) { playerContextId = id }
    fun setPlayerId(id: Long) { playerId = id }
    fun getBackendId(): Long = backendIdValue
    fun getPlayerContextId(): Long = playerContextId
    fun getPlayerId(): Long = playerId

    // ------------------------------------------------------------------------
    // Private — typed-call plumbing
    // ------------------------------------------------------------------------

    private suspend fun <R> invokeTyped(
        method: String,
        args: JsonElement,
        buffers: Array<ByteArray>?,
        handleKind: HandleKind,
        retSerializer: kotlinx.serialization.KSerializer<R>,
    ): BridgeRet<R> = withContext(Dispatchers.IO) {
        val handle = when (handleKind) {
            HandleKind.BACKEND -> backendIdValue
            HandleKind.PLAYER -> playerId
            HandleKind.PLAYER_CONTEXT -> playerContextId
            HandleKind.NONE -> 0L
        }
        val req = buildJsonObject {
            put("method", method)
            put("args", args)
            put("handle", handle)
        }
        val native = EaseBridge.call(req.toString(), buffers)
        val resp = json.decodeFromString(BridgeResponse.serializer(), native.payloadJson)
        BridgeRet(
            resp = resp,
            buffers = native.buffers,
            json = json,
            serializer = retSerializer,
            methodName = method,
            logFn = { msg -> logRaw("error", msg) },
        )
    }

    /** Encodes [arg] via [serializer]. Returns whatever shape the
     *  serializer produces — bare primitive for IDs/numbers/strings
     *  (matching Rust's `#[serde(transparent)]` expectation) or an
     *  object for data classes. */
    private fun <A> encodeArg(
        serializer: kotlinx.serialization.KSerializer<A>,
        arg: A,
    ): JsonElement = json.encodeToJsonElement(serializer, arg)
}
