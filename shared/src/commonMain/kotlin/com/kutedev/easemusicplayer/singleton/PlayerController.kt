package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.flow.StateFlow
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId

interface PlayerController {
    val sleepState: StateFlow<SleepModeState>
    fun getCurrentPosition(): Long
    fun getBufferedPosition(): Long
    fun play(id: MusicId, playlistId: PlaylistId)
    fun resume()
    fun pause()
    fun stop()
    fun playNext()
    fun playPrevious()
    fun seek(ms: ULong)
    fun scheduleSleep(newExpiredMs: Long)
    fun refreshPlaylistIfMatch(playlist: Playlist)
    fun cancelSleep()
}
