package com.kutedev.easemusicplayer.platform

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.BitmapFactory
import android.net.Uri
import android.widget.Toast
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap

lateinit var appContext: Context

fun initPlatformContext(context: Context) {
    appContext = context
}

actual fun platformShowToast(message: String) {
    Toast.makeText(appContext, message, Toast.LENGTH_SHORT).show()
}

actual fun platformOpenUrl(url: String) {
    val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).apply {
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
    appContext.startActivity(intent)
}

actual fun platformOpenFile(path: String) {
    val file = java.io.File(path)
    val uri = androidx.core.content.FileProvider.getUriForFile(
        appContext,
        "${appContext.packageName}.fileprovider",
        file
    )
    val intent = Intent(Intent.ACTION_VIEW).apply {
        setDataAndType(uri, "text/plain")
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
    appContext.startActivity(intent)
}

actual fun platformAppVersion(): String {
    return try {
        val packageInfo = appContext.packageManager.getPackageInfo(appContext.packageName, 0)
        packageInfo.versionName ?: "unknown"
    } catch (e: PackageManager.NameNotFoundException) {
        "unknown"
    }
}

actual fun decodeImageBitmap(bytes: ByteArray): ImageBitmap? {
    return BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
}
