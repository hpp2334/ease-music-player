package com.kutedev.easemusicplayer.core

import AsyncRuntimeAdapter
import android.annotation.SuppressLint
import android.app.Service
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import androidx.localbroadcastmanager.content.LocalBroadcastManager
import uniffi.ease_client_android.IPlayerDelegateForeign
import uniffi.ease_client_android.apiBuildBackend
import uniffi.ease_client_android.apiDestroyBackend
import uniffi.ease_client_android.apiSendBackendPlayerEvent
import uniffi.ease_client_android.apiStartBackend
import uniffi.ease_client_shared.ArgInitializeApp
import uniffi.ease_client_shared.PlayerDelegateEvent

const val BACKEND_STARTED_ACTION = "BACKEND_STARTED_ACTION"

private fun normalizePath(p: String): String {
    if (p.endsWith("/")) {
        return p;
    }
    return "$p/";
}

object BackendBridge {
    private const val STORAGE_PATH = "/"

    fun onCreate(context: android.content.Context, player: IPlayerDelegateForeign) {
        apiBuildBackend(
            AsyncRuntimeAdapter(),
            player
        )
        apiStartBackend(ArgInitializeApp(
            appDocumentDir = normalizePath(context.filesDir.absolutePath),
            appCacheDir = normalizePath(context.cacheDir.absolutePath),
            storagePath = STORAGE_PATH
        ))
    }

    fun onDestroy() {
        apiDestroyBackend()
    }

    fun sendPlayerEvent(evt: PlayerDelegateEvent) {
        apiSendBackendPlayerEvent(evt)
    }
}
