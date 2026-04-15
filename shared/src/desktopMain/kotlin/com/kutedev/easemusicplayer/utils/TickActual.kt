package com.kutedev.easemusicplayer.utils

import java.awt.EventQueue

internal actual fun postOnMainThread(f: () -> Unit) {
    EventQueue.invokeLater { f() }
}
