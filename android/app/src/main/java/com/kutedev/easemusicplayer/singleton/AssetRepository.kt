package com.kutedev.easemusicplayer.singleton

import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import com.kutedev.easemusicplayer.core.DataSourceKeyH
import javax.inject.Inject
import javax.inject.Singleton
import uniffi.ease_client_backend.ctGetAsset
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_schema.DataSourceKey

@Singleton
class AssetRepository @Inject constructor(private val bridge: Bridge) {
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
        val bm = BitmapFactory.decodeByteArray(buf, 0, buf.size) ?: return null
        val bitmap = bm.asImageBitmap()
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
