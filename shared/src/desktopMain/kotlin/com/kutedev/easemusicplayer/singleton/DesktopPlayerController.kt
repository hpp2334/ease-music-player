package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId

class DesktopPlayerController : PlayerController {
    override val sleepState: StateFlow<SleepModeState> = MutableStateFlow(SleepModeState())

    override fun getCurrentPosition(): Long = 0L
    override fun getBufferedPosition(): Long = 0L
    override fun play(id: MusicId, playlistId: PlaylistId) {}
    override fun resume() {}
    override fun pause() {}
    override fun stop() {}
    override fun playNext() {}
    override fun playPrevious() {}
    override fun seek(ms: ULong) {}
    override fun scheduleSleep(newExpiredMs: Long) {}
    override fun refreshPlaylistIfMatch(playlist: Playlist) {}
    override fun cancelSleep() {}
}
