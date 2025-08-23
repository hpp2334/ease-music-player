package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.repositories.ImportRepository
import com.kutedev.easemusicplayer.repositories.PlaylistRepository
import com.kutedev.easemusicplayer.utils.formatDuration
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgAddMusicsToPlaylist
import uniffi.ease_client_backend.ArgRemoveMusicFromPlaylist
import uniffi.ease_client_backend.MusicAbstract
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.PlaylistAbstract
import uniffi.ease_client_schema.PlaylistId
import uniffi.ease_client_backend.PlaylistMeta
import uniffi.ease_client_backend.StorageEntryType
import uniffi.ease_client_backend.ToAddMusicEntry
import uniffi.ease_client_backend.ctAddMusicsToPlaylist
import uniffi.ease_client_backend.ctGetPlaylist
import uniffi.ease_client_backend.ctRemoveMusicFromPlaylist
import uniffi.ease_client_backend.ctRemovePlaylist
import java.time.Duration
import javax.inject.Inject



@HiltViewModel
class PlaylistVM @Inject constructor(
    private val bridge: Bridge,
    private val playlistRepository: PlaylistRepository,
    private val importRepository: ImportRepository,
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
            reload()
            playlistRepository.playlists.collect {
                    _ -> reload()
            }
        }
    }

    fun remove() {
        playlistRepository.removePlaylist(_id)
    }

    fun removeMusic(id: MusicId) {
        viewModelScope.launch {
            ctRemoveMusicFromPlaylist(bridge.backend, ArgRemoveMusicFromPlaylist(
                playlistId = _id,
                musicId = id
            ))
            playlistRepository.reload()
        }
    }

    fun prepareImportMusics() {
        importRepository.prepare(listOf(StorageEntryType.MUSIC)) {
            entries ->
                viewModelScope.launch {
                    ctAddMusicsToPlaylist(bridge.backend, ArgAddMusicsToPlaylist(
                        id = _id,
                        entries = entries.map { entry -> ToAddMusicEntry(
                            entry = entry,
                            name = entry.name
                        ) }
                    ))

                    playlistRepository.reload()
                }
        }
    }

    fun openRemoveModal() {
        _removeModalOpen.value = true
    }

    fun closeRemoveModal() {
        _removeModalOpen.value = false
    }

    private suspend fun reload() {
        val playlist = ctGetPlaylist(bridge.backend, _id)
        if (playlist != null) {
            _playlist.value = playlist
        }
    }
}

fun PlaylistAbstract.durationStr(): String {
    return formatDuration(duration)
}

fun MusicAbstract.durationStr(): String {
    return formatDuration(meta.duration)
}