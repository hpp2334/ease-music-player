package com.kutedev.easemusicplayer.utils

import uniffi.ease_client_backend.Music
import java.time.Duration


fun formatDuration(duration: Duration?): String {
    if (duration != null) {
        val all = duration.toMillis()
        val h = all / 1000 / 60 / 60
        val m = all / 1000 / 60 % 60
        val s = all / 1000 % 60
        return "${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}"
    } else {
        return "--:--:--"
    }
}

fun formatDuration(music: Music?): String {
    return formatDuration(music?.meta?.duration)
}

fun toMusicDurationMs(music: Music?): ULong {
    return music?.meta?.duration?.toMillis()?.toULong() ?: 0uL
}

fun toMusicDurationMs(duration: Duration): ULong {
    return duration.toMillis().toULong()
}
