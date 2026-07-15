package com.kutedev.easemusicplayer.singleton

import com.kutedev.easemusicplayer.platform.AppPaths
import uniffi.ease_client_backend.ArgInitializeApp
import uniffi.ease_client_backend.Backend
import uniffi.ease_client_backend.createBackend
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_backend.easeLog
import java.lang.Exception
import kotlin.stackTrace


private fun normalizePath(p: String): String {
    if (p.endsWith("/")) {
        return p;
    }
    return "$p/";
}


class Bridge constructor(
    appPaths: AppPaths,
    private val toastRepository: ToastRepository
)  {
    private val _storagePath = "/"
    private var _isInit = false
    private val _arg = ArgInitializeApp(
        appDocumentDir = normalizePath(appPaths.documentDir),
        appCacheDir = normalizePath(appPaths.cacheDir),
        storagePath = _storagePath
    )
    private var _backend: Backend? = null

    private fun internal(): Backend {
        return _backend!!
    }

    suspend fun<R> run(block: suspend (backend: Backend) -> R): R? {
        try {
            return block(internal())
        } catch (e: Exception) {
            easeError("run bridge failed: $e")
            easeError("run bridge failed stacktrace: ${e.stackTraceToString()}")
            return null
        }
    }

    suspend fun<R> runRaw(block: suspend (backend: Backend) -> R): R {
        return block(internal())
    }

    fun<R> runSyncRaw(block: (backend: Backend) -> R): R {
        return block(internal())
    }

    fun<R> runSync(block: (backend: Backend) -> R): R? {
        try {
            return block(internal())
        } catch (e: Exception) {
            easeError("run bridge failed: $e")
            toastRepository.emitToast(e.toString())
            return null
        }
    }

    fun initialize() {
        if (_isInit) {
            return
        }
        _backend = createBackend(_arg)
        _backend!!.init();
        easeLog("bridge initialized")
        _isInit = true
    }

    fun destroy() {
        if (!_isInit) {
            return
        }
        _backend!!.deinit()
        _backend = null
        _isInit = false
        easeLog("bridge destroyed")
    }
}
