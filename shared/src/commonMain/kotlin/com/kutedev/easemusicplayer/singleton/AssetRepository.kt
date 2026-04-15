package com.kutedev.easemusicplayer.singleton

import androidx.compose.ui.graphics.ImageBitmap
import com.kutedev.easemusicplayer.core.DataSourceKeyH
import com.kutedev.easemusicplayer.platform.byteArrayToImageBitmap
import uniffi.ease_client_backend.ctGetAsset
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_schema.DataSourceKey

class AssetRepository(private val bridge: Bridge) {
    private val bufCache = HashMap<DataSourceKeyH, ByteArray>()
    private val bitmapCache = HashMap<DataSourceKeyH, ImageBitmap>()

    suspend fun load(key: DataSourceKey): ByteArray? {
        val keyH = DataSourceKeyH(key)
        bufCache[keyH]?.let {
            return it
        }

        return try {
            val buf = bridge.run { ctGetAsset(it, key) }
            if (buf != null) {
                bufCache[keyH] = buf
            }
            buf
        } catch (e: Exception) {
            easeError(e.toString())
            null
        }
    }

    suspend fun loadBitmap(key: DataSourceKey): ImageBitmap? {
        val keyH = DataSourceKeyH(key)
        bitmapCache[keyH]?.let {
            return it
        }

        val buf = load(key) ?: return null
        val bitmap = byteArrayToImageBitmap(buf) ?: return null
        bitmapCache[keyH] = bitmap
        return bitmap
    }

    fun get(key: DataSourceKey): ByteArray? {
        return bufCache[DataSourceKeyH(key)]
    }

    fun getBitmap(key: DataSourceKey): ImageBitmap? {
        return bitmapCache[DataSourceKeyH(key)]
    }
}
