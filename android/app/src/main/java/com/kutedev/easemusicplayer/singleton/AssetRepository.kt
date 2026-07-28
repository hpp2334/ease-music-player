package com.kutedev.easemusicplayer.singleton

import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import com.kutedev.easemusicplayer.core.DataSourceKeyH
import com.kutedev.easemusicplayer.singleton.types.DataSourceKey
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonObject
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AssetRepository @Inject constructor(private val bridge: Bridge) {
    private val bufCache = HashMap<DataSourceKeyH, ByteArray>()
    private val bitmapCache = HashMap<DataSourceKeyH, ImageBitmap>()

    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        classDiscriminator = "kind"
    }

    suspend fun load(key: DataSourceKey): ByteArray? {
        val keyH = DataSourceKeyH(key)
        bufCache[keyH]?.let { return it }

        val args = buildJsonObject {
            put("key", json.encodeToJsonElement(DataSourceKey.serializer(), key))
        }
        val buf = bridge.callRaw("asset.get", args).unwrapOrNull()?.getBuffer(0)
        if (buf != null) {
            bufCache[keyH] = buf
        }
        return buf
    }

    suspend fun loadBitmap(key: DataSourceKey): ImageBitmap? {
        val keyH = DataSourceKeyH(key)
        bitmapCache[keyH]?.let { return it }

        val buf = load(key) ?: return null
        val bm = BitmapFactory.decodeByteArray(buf, 0, buf.size) ?: return null
        val bitmap = bm.asImageBitmap()
        bitmapCache[keyH] = bitmap
        return bitmap
    }

    fun get(key: DataSourceKey): ByteArray? =
        bufCache[DataSourceKeyH(key)]

    fun getBitmap(key: DataSourceKey): ImageBitmap? =
        bitmapCache[DataSourceKeyH(key)]
}
