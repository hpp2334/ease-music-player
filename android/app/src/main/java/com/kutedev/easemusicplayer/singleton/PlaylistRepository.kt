package com.kutedev.easemusicplayer.singleton

import kotlinx.collections.immutable.persistentListOf
import kotlinx.collections.immutable.toPersistentList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.time.debounce
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import com.kutedev.easemusicplayer.singleton.types.AddedMusic
import com.kutedev.easemusicplayer.singleton.types.ArgCreatePlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgRemoveMusicFromPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgReorderPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgUpdateMusicDuration
import com.kutedev.easemusicplayer.singleton.types.ArgUpdatePlaylist
import com.kutedev.easemusicplayer.singleton.types.PlaylistAbstract
import com.kutedev.easemusicplayer.singleton.types.MusicId
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import com.kutedev.easemusicplayer.singleton.types.RetCreatePlaylist
import java.time.Duration
import javax.inject.Inject
import javax.inject.Singleton


@Singleton
class PlaylistRepository @Inject constructor(
    private val bridge: Bridge,
    private val storageRepository: StorageRepository,
    private val _scope: CoroutineScope,
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

    fun createPlaylist(arg: ArgCreatePlaylist) {
        _scope.launch {
            val created = bridge.call(BridgeMethods.Playlist.CREATE, arg)
                .unwrapOrNull()?.payload
            if ((created?.musicIds?.size ?: 0) > 0) {
                requestTotalDuration(created!!.musicIds)
            }
            reload()
        }
    }

    fun editPlaylist(arg: ArgUpdatePlaylist) {
        _scope.launch {
            bridge.call(BridgeMethods.Playlist.UPDATE, arg).unwrapOrNull()
            reload()
        }
    }

    fun removePlaylist(id: PlaylistId) {
        _scope.launch {
            _preRemovePlaylistEvent.emit(id)
            bridge.call(BridgeMethods.Playlist.REMOVE, id).unwrapOrNull()
            reload()
        }
    }

    fun requestTotalDuration(added: List<AddedMusic>) {
        for (item in added) {
            if (!item.existed) {
                _scope.launch { probeAndPersistDuration(item.id) }
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
            val arg = ArgReorderPlaylist(
                id = from.meta.id,
                a = a?.meta?.id,
                b = b?.meta?.id,
            )
            bridge.call(BridgeMethods.Playlist.REORDER, arg).unwrapOrNull()
            scheduleReload()
        }
    }


    suspend fun removeMusic(playlistId: PlaylistId, musicId: MusicId) {
        val arg = ArgRemoveMusicFromPlaylist(
            playlistId = playlistId,
            musicId = musicId,
        )
        _preRemoveMusicEvent.emit(arg)
        bridge.call(BridgeMethods.Playlist.REMOVE_MUSIC, arg).unwrapOrNull()
        reload()
    }

    /**
     * Probes [id]'s duration via `player.probeDurationMs` (no playback,
     * no output device — uses [cantode::probe_metadata]) and persists
     * the result via `music.updateDuration`. Emits
     * [_syncedTotalDuration] so [PlaylistVM] reloads.
     *
     * Silently no-ops if the cantode player context isn't set up yet
     * (early in app startup) or the probe fails — the existing
     * `player.loadMusic` writeback hook will fill in the duration on
     * first play as a fallback.
     */
    private suspend fun probeAndPersistDuration(id: MusicId) {
        val contextHandle = bridge.getPlayerContextId()
        if (contextHandle < 0L) return
        val args = buildJsonObject {
            put("contextHandle", contextHandle)
            put("backendHandle", bridge.getBackendId())
            put("musicId", id.value)
        }
        val payload = bridge.callRaw("player.probeDurationMs", args)
            .unwrapOrNull()?.rawPayloadJson ?: return
        if (payload is JsonNull) return
        val durMs = payload.jsonPrimitive.content.toLong()
        bridge.call(
            BridgeMethods.Music.UPDATE_DURATION,
            ArgUpdateMusicDuration(id = id, duration = durMs),
        ).unwrapOrNull()
        _syncedTotalDuration.emit(id)
    }

    fun scheduleReload() {
        _scope.launch {
            _debouncedReloadEvent.emit(Unit)
        }
    }

    suspend fun reload() {
        val list: List<PlaylistAbstract>? = bridge.call(BridgeMethods.Playlist.LIST).unwrapOrNull()?.payload
        _playlists.value = list?.toPersistentList() ?: persistentListOf()
    }
}
