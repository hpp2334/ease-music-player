package com.kutedev.easemusicplayer.platform

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import org.jetbrains.skia.Image

actual fun getAppDocumentDir(): String {
    return "${System.getProperty("user.home")}/.easemusicplayer"
}

actual fun getAppCacheDir(): String {
    return "${System.getProperty("user.home")}/.easemusicplayer/cache"
}

actual fun byteArrayToImageBitmap(bytes: ByteArray): ImageBitmap? {
    return try {
        Image.makeFromEncoded(bytes).toComposeImageBitmap()
    } catch (e: Exception) {
        null
    }
}
