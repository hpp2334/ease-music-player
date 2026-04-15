package com.kutedev.easemusicplayer.utils

internal actual fun postOnMainThread(f: () -> Unit) {
    android.os.Handler(android.os.Looper.getMainLooper()).post {
        f()
    }
}
