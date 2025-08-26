package com.kutedev.easemusicplayer.viewmodels

import android.content.Context
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.core.syncMetadataUtil
import com.kutedev.easemusicplayer.singleton.ImportRepository
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.utils.formatDuration
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Semaphore
import uniffi.ease_client_backend.AddedMusic
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
import uniffi.ease_client_backend.ctsGetMusicAbstract
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
                createdTime = Duration.ofMillis(0L),
                order = listOf(0u)
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
        viewModelScope.launch {
            playlistRepository.syncedTotalDuration.collect {
                reload()
            }
        }
    }

    fun remove() {
        playlistRepository.removePlaylist(_id)
    }

    fun removeMusic(id: MusicId) {
        viewModelScope.launch {
            bridge.run { backend -> ctRemoveMusicFromPlaylist(backend, ArgRemoveMusicFromPlaylist(
                playlistId = _id,
                musicId = id
            ))}
            playlistRepository.reload()
        }
    }

    fun prepareImportMusics(context: Context) {
        importRepository.prepare(listOf(StorageEntryType.MUSIC)) {
            entries ->
                viewModelScope.launch {
                    val added = bridge.run { backend ->
                        ctAddMusicsToPlaylist(
                            backend, ArgAddMusicsToPlaylist(
                            id = _id,
                            entries = entries.map { entry ->
                                ToAddMusicEntry(
                                    entry = entry,
                                    name = entry.name
                                )
                            }
                    ))} ?: emptyList()
                    playlistRepository.requestTotalDuration(context, added)
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
        val playlist = bridge.run { backend -> ctGetPlaylist(backend, _id) }
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