package com.kutedev.easemusicplayer.viewmodels

import android.content.Context
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.BridgeMethods
import com.kutedev.easemusicplayer.singleton.ImportRepository
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import com.kutedev.easemusicplayer.utils.formatDuration
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.collections.immutable.persistentListOf
import kotlinx.collections.immutable.toPersistentList
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.time.debounce
import com.kutedev.easemusicplayer.singleton.types.AddedMusic
import com.kutedev.easemusicplayer.singleton.types.ArgAddMusicsToPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgRemoveMusicFromPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgReorderMusic
import com.kutedev.easemusicplayer.singleton.types.ArgReorderPlaylist
import com.kutedev.easemusicplayer.singleton.types.MusicAbstract
import com.kutedev.easemusicplayer.singleton.types.MusicId
import com.kutedev.easemusicplayer.singleton.types.Playlist
import com.kutedev.easemusicplayer.singleton.types.PlaylistAbstract
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import com.kutedev.easemusicplayer.singleton.types.PlaylistMeta
import com.kutedev.easemusicplayer.singleton.types.StorageEntryType
import com.kutedev.easemusicplayer.singleton.types.ToAddMusicEntry
import java.time.Duration
import javax.inject.Inject
import kotlin.time.toKotlinDuration

private fun defaultPlaylistAbstract(): PlaylistAbstract {
    return PlaylistAbstract(
        meta = PlaylistMeta(
            id = PlaylistId(0),
            title = "",
            cover = null,
            showCover = null,
            createdTime = 0L,
            order = listOf(0L)
        ),
        musicCount = 0uL,
        duration = null
    )
}

@HiltViewModel
class PlaylistVM @Inject constructor(
    private val bridge: Bridge,
    private val playlistRepository: PlaylistRepository,
    private val storageRepository: StorageRepository,
    private val importRepository: ImportRepository,
    private val playerControllerRepository: PlayerControllerRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {
    private val _id: PlaylistId = PlaylistId(savedStateHandle["id"]!!)
    private val _removeModalOpen = MutableStateFlow(false)
    private val _playlistAbstr = MutableStateFlow(defaultPlaylistAbstract())
    private val _playlistMusics = MutableStateFlow(persistentListOf<MusicAbstract>())
    val removeModalOpen = _removeModalOpen.asStateFlow()
    val playlistAbstr = _playlistAbstr.asStateFlow()
    val playlistMusics = _playlistMusics.asStateFlow()

    init {
        viewModelScope.launch {
            reload()
            playlistRepository.playlists.collect {
                    _ -> reload()
            }
        }
        viewModelScope.launch {
            playlistRepository.syncedTotalDuration.debounce(Duration.ofMillis(500)).collect {
                reload()
            }
        }
        viewModelScope.launch {
            storageRepository.onRemoveStorageEvent.collect {
                reload()
            }
        }
    }

    fun remove() {
        playlistRepository.removePlaylist(_id)
    }

    fun removeMusic(id: MusicId) {
        viewModelScope.launch {
            playlistRepository.removeMusic(_id, id)
        }
    }

    fun prepareImportMusics(context: Context) {
        importRepository.prepare(listOf(StorageEntryType.MUSIC)) { entries ->
            viewModelScope.launch {
                val arg = ArgAddMusicsToPlaylist(
                    id = _id,
                    entries = entries.map { entry ->
                        ToAddMusicEntry(entry = entry, name = entry.name)
                    },
                )
                val added: List<AddedMusic> = bridge.call(BridgeMethods.Playlist.ADD_MUSICS, arg)
                    .unwrapOrNull()?.payload ?: emptyList()
                playlistRepository.requestTotalDuration(context, added)
                playlistRepository.reload()
            }
        }
    }

    fun musicMoveTo(fromIndex: Int, toIndex: Int) {
        val from = _playlistMusics.value.getOrNull(fromIndex) ?: return

        _playlistMusics.value = _playlistMusics.value
            .removeAt(fromIndex)
            .add(toIndex, from)

        val a = _playlistMusics.value.getOrNull(toIndex - 1)
        val b = _playlistMusics.value.getOrNull(toIndex + 1)

        viewModelScope.launch {
            val arg = ArgReorderMusic(
                playlistId = _playlistAbstr.value.meta.id,
                id = from.meta.id,
                a = a?.meta?.id,
                b = b?.meta?.id,
            )
            bridge.call(BridgeMethods.Playlist.REORDER_MUSIC, arg).unwrapOrNull()
            playlistRepository.scheduleReload()
            reload()
        }
    }

    fun openRemoveModal() {
        _removeModalOpen.value = true
    }

    fun closeRemoveModal() {
        _removeModalOpen.value = false
    }

    private suspend fun reload() {
        val playlist: Playlist? = bridge.call(BridgeMethods.Playlist.GET, _id).unwrapOrNull()?.payload
        if (playlist != null) {
            _playlistAbstr.value = playlist.abstr
            _playlistMusics.value = playlist.musics.toPersistentList()
            playerControllerRepository.refreshPlaylistIfMatch(playlist)
        } else {
            _playlistAbstr.value = defaultPlaylistAbstract()
            _playlistMusics.value = persistentListOf()
        }
    }
}

fun PlaylistAbstract.durationStr(): String {
    return formatDuration(duration)
}

fun MusicAbstract.durationStr(): String {
    return formatDuration(meta.duration)
}
