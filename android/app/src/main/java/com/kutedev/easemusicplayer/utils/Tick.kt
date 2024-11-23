package com.kutedev.easemusicplayer.utils

fun nextTickOnMain(f: () -> Unit) {
    android.os.Handler(android.os.Looper.getMainLooper()).post {
        f()
    }
}