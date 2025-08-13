package com.kutedev.easemusicplayer.core

import android.annotation.SuppressLint
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.app.Service.START_NOT_STICKY
import android.content.Context
import android.content.Intent
import android.os.IBinder
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.core.app.NotificationCompat
import androidx.localbroadcastmanager.content.LocalBroadcastManager
import dagger.hilt.android.qualifiers.ApplicationContext
import uniffi.ease_client_backend.ArgInitializeApp
import uniffi.ease_client_backend.Backend
import uniffi.ease_client_backend.createBackend
import javax.inject.Inject
import javax.inject.Singleton


private fun normalizePath(p: String): String {
    if (p.endsWith("/")) {
        return p;
    }
    return "$p/";
}


@Singleton
class Bridge @Inject constructor(
    @ApplicationContext cx: Context
)  {
    private val _storagePath = "/"
    private val _backend: Backend = createBackend(
        ArgInitializeApp(
            appDocumentDir = normalizePath(cx.filesDir.absolutePath),
            appCacheDir = normalizePath(cx.cacheDir.absolutePath),
            storagePath = _storagePath
        )
    )

    fun initialize() {
        _backend.init();
    }
}

val LocalBridge = compositionLocalOf<Bridge> {
    error("No UIBridge provided")
}

@Composable
fun BridgeProvider(
    bridge: Bridge,
    content: @Composable () -> Unit
) {
    CompositionLocalProvider(LocalBridge provides bridge) {
        content()
    }
}
