package com.kutedev.easemusicplayer.singleton

import android.content.Context
import kotlinx.collections.immutable.persistentListOf
import kotlinx.collections.immutable.toPersistentList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.time.debounce
import uniffi.ease_client_backend.AddedMusic
import uniffi.ease_client_backend.ArgCreatePlaylist
import uniffi.ease_client_backend.ArgRemoveMusicFromPlaylist
import uniffi.ease_client_backend.ArgReorderPlaylist
import uniffi.ease_client_backend.ArgUpdatePlaylist
import uniffi.ease_client_backend.PlaylistAbstract
import uniffi.ease_client_backend.ctCreatePlaylist
import uniffi.ease_client_backend.ctListPlaylist
import uniffi.ease_client_backend.ctRemoveMusicFromPlaylist
import uniffi.ease_client_backend.ctRemovePlaylist
import uniffi.ease_client_backend.ctUpdatePlaylist
import uniffi.ease_client_backend.ctsGetMusicAbstract
import uniffi.ease_client_backend.ctsReorderPlaylist
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId
import java.time.Duration
import javax.inject.Inject
import javax.inject.Singleton


@Singleton
class PlaylistRepository @Inject constructor(
    private val bridge: Bridge,
    private val storageRepository: StorageRepository,
    private val _scope: CoroutineScope
) {
    private val _playlists = MutableStateFlow(persistentListOf<PlaylistAbstract>())
    private val _syncedTotalDuration = MutableSharedFlow<MusicId>()
    private val _debouncedReloadEvent = MutableSharedFlow<Unit>()
    private val _preRemovePlaylistEvent = MutableSharedFlow<PlaylistId>()
    private val _preRemoveMusicEvent = MutableSharedFlow<ArgRemoveMusicFromPlaylist>()

    val playlists = _playlists.asStateFlow()
    val syncedTotalDuration = _syncedTotalDuration.asSharedFlow()
    val preRemovePlaylistEvent = _preRemovePlaylistEvent.asSharedFlow()
    val preRemoveMusicEvent = _preRemoveMusicEvent.asSharedFlow()

    init {
        _scope.launch {
            _debouncedReloadEvent.debounce(Duration.ofMillis(500)).collect {
                reload()
            }
        }
        _scope.launch {
            storageRepository.onRemoveStorageEvent.collect {
                reload()
            }
        }
    }

    fun createPlaylist(context: Context, arg: ArgCreatePlaylist) {
        _scope.launch {
            val created = bridge.run { ctCreatePlaylist(it, arg) }
            if ((created?.musicIds?.size ?: 0) > 0) {
                requestTotalDuration(context, created!!.musicIds)
            }
            reload()
        }
    }

    fun editPlaylist(arg: ArgUpdatePlaylist) {
        _scope.launch {
            bridge.run { ctUpdatePlaylist(it, arg) }
            reload()
        }
    }

    fun removePlaylist(id: PlaylistId) {
        _scope.launch {
            _preRemovePlaylistEvent.emit(id)
            bridge.run { ctRemovePlaylist(it, id) }
            reload()
        }
    }

    fun requestTotalDuration(context: Context, added: List<AddedMusic>) {
        for (item in added) {
            if (!item.existed) {
                requestTotalDuration(context, item.id)
            }
        }
    }

    fun playlistMoveTo(fromIndex: Int, toIndex: Int) {
        val from = _playlists.value.getOrNull(fromIndex) ?: return

        _playlists.value = _playlists.value
            .removeAt(fromIndex)
            .add(toIndex, from)

        val a = _playlists.value.getOrNull(toIndex - 1)
        val b = _playlists.value.getOrNull(toIndex + 1)

        _scope.launch {
            bridge.runSync { ctsReorderPlaylist(it, ArgReorderPlaylist(
                id = from.meta.id,
                a = a?.meta?.id,
                b = b?.meta?.id))
            }
            scheduleReload()
        }
    }


    suspend fun removeMusic(playlistId: PlaylistId, musicId: MusicId) {
        val arg = ArgRemoveMusicFromPlaylist(
            playlistId = playlistId,
            musicId = musicId
        )
        _preRemoveMusicEvent.emit(arg)
        bridge.run { backend -> ctRemoveMusicFromPlaylist(backend, arg)}

        reload()
    }

    /**
     * Proactively probe + persist the duration of [id] without playing it.
     *
     * TODO(v0.5): wire this to `ctPlayerProbeDurationMs` via a shared
     * `PlayerContextHandle`. For v0.4 we leave this as a no-op — music
     * durations are lazily filled in on first play via the backend's
     * `ctPlayerLoadMusic` writeback hook. The user-visible effect is
     * that newly-imported tracks show no duration in the playlist UI
     * until they're played once.
     */
    @Suppress("UNUSED_PARAMETER")
    private fun requestTotalDuration(context: Context, id: MusicId) {
        // No-op — see KDoc.
    }

    fun scheduleReload() {
        _scope.launch {
            _debouncedReloadEvent.emit(Unit)
        }
    }

    suspend fun reload() {
        _playlists.value = bridge.run { ctListPlaylist(it).toPersistentList() } ?: persistentListOf()
    }
}