package com.kutedev.easemusicplayer.core

import AsyncRuntimeAdapter
import android.annotation.SuppressLint
import android.app.Service
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import uniffi.ease_client_android.IPlayerDelegateForeign
import uniffi.ease_client_android.apiBuildBackend
import uniffi.ease_client_android.apiDestroyBackend
import uniffi.ease_client_android.apiSendBackendPlayerEvent
import uniffi.ease_client_android.apiStartBackend
import uniffi.ease_client_shared.ArgInitializeApp
import uniffi.ease_client_shared.PlayerDelegateEvent

const val BACKEND_STARTED_ACTION = "BACKEND_STARTED_ACTION"

object BackendBridge {
    @SuppressLint("StaticFieldLeak")
    private const val SCHEMA_VERSION = 1u
    private const val STORAGE_PATH = "/"

    fun onCreate(context: android.content.Context, player: IPlayerDelegateForeign) {
        apiBuildBackend(
            AsyncRuntimeAdapter(),
            player
        )
        apiStartBackend(ArgInitializeApp(
            context.filesDir.absolutePath,
            SCHEMA_VERSION,
            STORAGE_PATH
        ))
    }

    fun onDestroy() {
        apiDestroyBackend()
    }

    fun sendPlayerEvent(evt: PlayerDelegateEvent) {
        apiSendBackendPlayerEvent(evt)
    }
}
