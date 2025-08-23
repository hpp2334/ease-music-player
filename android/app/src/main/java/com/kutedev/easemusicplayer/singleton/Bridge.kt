package com.kutedev.easemusicplayer.singleton

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import dagger.hilt.android.qualifiers.ApplicationContext
import uniffi.ease_client_backend.ArgInitializeApp
import uniffi.ease_client_backend.Backend
import uniffi.ease_client_backend.createBackend
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_backend.easeLog
import java.lang.Exception
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
    @ApplicationContext cx: Context,
    private val toastRepository: ToastRepository
)  {
    private val _storagePath = "/"
    private val _backend: Backend = createBackend(
        ArgInitializeApp(
            appDocumentDir = normalizePath(cx.filesDir.absolutePath),
            appCacheDir = normalizePath(cx.cacheDir.absolutePath),
            storagePath = _storagePath
        )
    )

    suspend fun<R> run(block: suspend (backend: Backend) -> R): R? {
        try {
            return block(_backend)
        } catch (e: Exception) {
            easeError("run bridge failed: $e")
            return null
        }
    }

    suspend fun<R> runRaw(block: suspend (backend: Backend) -> R): R {
        return block(_backend)
    }

    fun<R> runSync(block: (backend: Backend) -> R): R? {
        try {
            return block(_backend)
        } catch (e: Exception) {
            easeError("run bridge failed: $e")
            toastRepository.emitToast(e.toString())
            return null
        }
    }

    fun initialize() {
        _backend.init();
        easeLog("bridge initialized")
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
