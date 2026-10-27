package com.kutedev.easemusicplayer.singleton

import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import com.kutedev.easemusicplayer.core.DataSourceKeyH
import com.kutedev.easemusicplayer.singleton.types.DataSourceKey
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Terminal outcome of a bitmap load. [Failed] is cached like [Loaded] —
 * the same bytes decode (or fetch) the same way every time, so retrying
 * per composition entry would only re-decode garbage or re-hit the
 * network. Renderers fall back to the no-cover default art on [Failed].
 */
sealed interface AssetBitmap {
    data class Loaded(val bitmap: ImageBitmap) : AssetBitmap
    data object Failed : AssetBitmap
}

@Singleton
class AssetRepository @Inject constructor(private val bridge: Bridge) {
    private val bufCache = HashMap<DataSourceKeyH, ByteArray>()
    private val bitmapCache = HashMap<DataSourceKeyH, AssetBitmap>()

    // Negative cache for `asset.get` answering "no bytes" (empty cover
    // blob under a set id, missing remote entry) — a deterministic miss,
    // so it is terminal. Bridge ERRORS stay uncached: transient, and the
    // next composition entry retries them.
    private val noBytesCache = HashSet<DataSourceKeyH>()

    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        classDiscriminator = "kind"
    }

    suspend fun load(key: DataSourceKey): ByteArray? {
        val keyH = DataSourceKeyH(key)
        bufCache[keyH]?.let { return it }
        if (keyH in noBytesCache) return null

        // The tagged enum IS the args object — the Rust `asset.get` arm
        // deserializes `req.args` directly as `DataSourceKey` (same
        // bare-value convention as `music.get`/`playlist.get`). Wrapping
        // it in a `{"key": …}` object breaks the `#[serde(tag = "kind")]`
        // deserialization ("missing field `kind`") and every cover fetch
        // fails.
        val args = json.encodeToJsonElement(DataSourceKey.serializer(), key)
        val ret = bridge.callRaw("asset.get", args)
        if (!ret.isSuccess) {
            // unwrapOrNull runs the standard error log; a transport
            // failure is transient, so nothing is cached here.
            ret.unwrapOrNull()
            return null
        }
        val buf = ret.unwrapOrThrow().getBuffer(0)
        if (buf == null) {
            noBytesCache.add(keyH)
        } else {
            bufCache[keyH] = buf
        }
        return buf
    }

    suspend fun loadBitmap(key: DataSourceKey): AssetBitmap {
        val keyH = DataSourceKeyH(key)
        bitmapCache[keyH]?.let { return it }

        val buf = load(key)
        if (buf == null) {
            // Cache the deterministic miss ("no bytes"); a transient
            // bridge error stays uncached (see noBytesCache).
            if (keyH in noBytesCache) {
                val failed = AssetBitmap.Failed
                bitmapCache[keyH] = failed
                return failed
            }
            return AssetBitmap.Failed
        }

        val bitmap = try {
            BitmapFactory.decodeByteArray(buf, 0, buf.size)?.asImageBitmap()
        } catch (t: Throwable) {
            // BitmapFactory THROWS (rather than returning null) on OOM
            // for huge dimensions — a legal 10000×10000 embedded PNG is
            // enough. Treat it like any other decode failure.
            bridge.logRaw("warn", "asset decode threw for $key: $t")
            null
        }
        if (bitmap == null) {
            // The failure would otherwise be invisible: log it where the
            // backend log (and logcat) can answer "why is this cover
            // blank".
            bridge.logRaw(
                "warn",
                "asset decode failed for $key (${buf.size} bytes) — undecodable image data",
            )
            val failed = AssetBitmap.Failed
            bitmapCache[keyH] = failed
            return failed
        }
        val loaded = AssetBitmap.Loaded(bitmap)
        bitmapCache[keyH] = loaded
        return loaded
    }

    fun get(key: DataSourceKey): ByteArray? =
        bufCache[DataSourceKeyH(key)]

    fun getBitmap(key: DataSourceKey): ImageBitmap? =
        (bitmapCache[DataSourceKeyH(key)] as? AssetBitmap.Loaded)?.bitmap

    /** Cached terminal outcome, or null while nothing has been resolved. */
    fun getCachedAsset(key: DataSourceKey): AssetBitmap? =
        bitmapCache[DataSourceKeyH(key)]
}
