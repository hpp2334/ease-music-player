package com.kutedev.easemusicplayer.utils

internal expect fun postOnMainThread(f: () -> Unit)

fun nextTickOnMain(f: () -> Unit) {
    postOnMainThread { f() }
}