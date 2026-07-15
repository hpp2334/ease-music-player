package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.flow.StateFlow
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId

data class SleepModeState(
    val enabled: Boolean = false,
    val expiredMs: Long = 0L
)

interface PlayerController {
    val sleepState: StateFlow<SleepModeState>
    
    fun play(id: MusicId, playlistId: PlaylistId)
    fun resume()
    fun pause()
    fun stop()
    fun playNext()
    fun playPrevious()
    fun seek(ms: ULong)
    fun getCurrentPosition(): Long
    fun getBufferedPosition(): Long
    fun scheduleSleep(expiredMs: Long)
    fun cancelSleep()
    fun refreshPlaylistIfMatch(playlist: Playlist)
    fun remove()
}

interface PermissionManager {
    val hasStoragePermission: StateFlow<Boolean>
    fun requestStoragePermission()
}
