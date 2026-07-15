package com.kutedev.easemusicplayer.platform

import androidx.compose.ui.graphics.ImageBitmap

data class AppPaths(
    val documentDir: String,
    val cacheDir: String,
)

expect fun platformShowToast(message: String)
expect fun platformOpenUrl(url: String)
expect fun platformAppVersion(): String
expect fun decodeImageBitmap(bytes: ByteArray): ImageBitmap?
expect fun platformOpenFile(path: String)
