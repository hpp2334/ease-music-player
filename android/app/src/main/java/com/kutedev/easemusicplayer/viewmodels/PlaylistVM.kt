package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.repositories.PlaylistRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgRemoveMusicFromPlaylist
import uniffi.ease_client_backend.MusicAbstract
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.PlaylistAbstract
import uniffi.ease_client_schema.PlaylistId
import uniffi.ease_client_backend.PlaylistMeta
import uniffi.ease_client_backend.ctGetPlaylist
import uniffi.ease_client_backend.ctRemoveMusicFromPlaylist
import uniffi.ease_client_backend.ctRemovePlaylist
import java.time.Duration
import javax.inject.Inject



@HiltViewModel
class PlaylistVM @Inject constructor(
    private val bridge: Bridge,
    private val playlistRepository: PlaylistRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {
    private val _id: PlaylistId = PlaylistId(savedStateHandle["id"]!!)
    private val _removeModalOpen = MutableStateFlow(false)
    private val _playlist = MutableStateFlow(Playlist(
        abstr = PlaylistAbstract(
            meta = PlaylistMeta(
                id = PlaylistId(0),
                title = "",
                cover = null,
                showCover = null,
                createdTime = Duration.ofMillis(0L)
            ),
            musicCount = 0uL,
            duration = null
        ),
        musics = emptyList()
    ))
    val removeModalOpen = _removeModalOpen.asStateFlow()
    val playlist = _playlist.asStateFlow()

    init {
        viewModelScope.launch {
            val playlist = ctGetPlaylist(bridge.backend, _id)
            if (playlist != null) {
                _playlist.value = playlist
            }
        }
    }

    fun remove() {
        viewModelScope.launch {
            ctRemovePlaylist(bridge.backend, _id)
        }
    }

    fun removeMusic(id: MusicId) {
        viewModelScope.launch {
            ctRemoveMusicFromPlaylist(bridge.backend, ArgRemoveMusicFromPlaylist(
                playlistId = _id,
                musicId = id
            ))
        }
    }

    fun openRemoveModal() {
        _removeModalOpen.value = true
    }

    fun closeRemoveModal() {
        _removeModalOpen.value = false
    }
}

private fun _durationStr(duration: Duration?): String {
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

fun PlaylistAbstract.durationStr(): String {
    return _durationStr(duration)
}

fun MusicAbstract.durationStr(): String {
    return _durationStr(meta.duration)
}