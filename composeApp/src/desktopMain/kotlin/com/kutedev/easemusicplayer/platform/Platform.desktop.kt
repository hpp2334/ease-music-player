package com.kutedev.easemusicplayer.platform

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import java.awt.Desktop
import java.awt.image.BufferedImage
import java.io.ByteArrayInputStream
import java.net.URI
import javax.imageio.ImageIO

actual fun platformShowToast(message: String) {
    println("[Toast] $message")
}

actual fun platformOpenUrl(url: String) {
    if (Desktop.isDesktopSupported()) {
        Desktop.getDesktop().browse(URI(url))
    }
}

actual fun platformOpenFile(path: String) {
    if (Desktop.isDesktopSupported()) {
        Desktop.getDesktop().open(java.io.File(path))
    }
}

actual fun platformAppVersion(): String {
    return "0.4.0-dev"
}

actual fun decodeImageBitmap(bytes: ByteArray): ImageBitmap? {
    return try {
        val image: BufferedImage? = ImageIO.read(ByteArrayInputStream(bytes))
        image?.toComposeImageBitmap()
    } catch (e: Exception) {
        null
    }
}
