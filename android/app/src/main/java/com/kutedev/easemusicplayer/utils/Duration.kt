package com.kutedev.easemusicplayer.utils

import com.kutedev.easemusicplayer.singleton.types.Music


fun formatDuration(durationMs: Long?): String {
    if (durationMs != null) {
        val all = durationMs
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
    return music?.meta?.duration?.toULong() ?: 0uL
}

fun toMusicDurationMs(durationMs: Long): ULong {
    return durationMs.toULong()
}
