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
import com.kutedev.easemusicplayer.singleton.types.ArgCreatePlaylistGroup
import com.kutedev.easemusicplayer.singleton.types.ArgEnsurePlaylistGroups
import com.kutedev.easemusicplayer.singleton.types.ArgMovePlaylistToGroup
import com.kutedev.easemusicplayer.singleton.types.ArgRemoveMusicFromPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgReorderPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgReorderPlaylistGroup
import com.kutedev.easemusicplayer.singleton.types.ArgRemovePlaylistGroup
import com.kutedev.easemusicplayer.singleton.types.ArgSetPlaylistGroupExpanded
import com.kutedev.easemusicplayer.singleton.types.ArgUpdateMusicDuration
import com.kutedev.easemusicplayer.singleton.types.ArgUpdatePlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgUpdatePlaylistGroup
import com.kutedev.easemusicplayer.singleton.types.PlaylistAbstract
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupMeta
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupId
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
    private val _groups = MutableStateFlow(persistentListOf<PlaylistGroupMeta>())
    private val _syncedTotalDuration = MutableSharedFlow<MusicId>()
    private val _debouncedReloadEvent = MutableSharedFlow<Unit>()
    private val _preRemovePlaylistEvent = MutableSharedFlow<PlaylistId>()
    private val _preRemoveMusicEvent = MutableSharedFlow<ArgRemoveMusicFromPlaylist>()

    val playlists = _playlists.asStateFlow()
    val groups = _groups.asStateFlow()
    val syncedTotalDuration = _syncedTotalDuration.asSharedFlow()
    val preRemovePlaylistEvent = _preRemovePlaylistEvent.asSharedFlow()
    val preRemoveMusicEvent = _preRemoveMusicEvent.asSharedFlow()

    /**
     * Localized title for the Default group created by the
     * `playlistGroup.ensureDefault` self-heal (set from the activity's
     * resolved locale before the first reload; null = Rust's "Default").
     */
    @Volatile
    private var defaultGroupTitle: String? = null

    fun setDefaultGroupTitle(title: String) {
        defaultGroupTitle = title
    }

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

    /**
     * Optimistic within-group playlist reorder: [newSlice] is the
     * group's playlist list after the move, [movedId] the dragged
     * playlist. The flat list is rebuilt with the slice re-inserted
     * contiguously; the bridge gets neighbor anchors within the slice.
     */
    fun applyGroupPlaylistOrder(
        groupId: PlaylistGroupId,
        newSlice: List<PlaylistAbstract>,
        movedId: PlaylistId,
    ) {
        val old = _playlists.value
        val firstIdx = old.indexOfFirst { it.meta.groupId == groupId }
        val item = newSlice.firstOrNull { it.meta.id == movedId } ?: return
        if (firstIdx < 0) {
            return
        }

        val nonGroup = old.filter { it.meta.groupId != groupId }
        val rebuilt = nonGroup.subList(0, firstIdx) + newSlice +
            nonGroup.subList(firstIdx, nonGroup.size)
        _playlists.value = rebuilt.toPersistentList()

        val idx = newSlice.indexOfFirst { it.meta.id == movedId }
        val a = newSlice.getOrNull(idx - 1)?.meta?.id
        val b = newSlice.getOrNull(idx + 1)?.meta?.id

        _scope.launch {
            val arg = ArgReorderPlaylist(
                id = item.meta.id,
                a = a,
                b = b,
            )
            bridge.call(BridgeMethods.Playlist.REORDER, arg).unwrapOrNull()
            scheduleReload()
        }
    }

    /**
     * Optimistic cross-group playlist move (drag-drop): [newToSlice]
     * is the destination group's playlist list with the moved playlist
     * already inserted at the drop position. The flat list is rebuilt
     * group-slice by group-slice; the bridge gets one atomic
     * `playlistGroup.movePlaylist` call (group change + landing order
     * from destination neighbors).
     */
    fun applyPlaylistGroupMove(
        playlistId: PlaylistId,
        toGroupId: PlaylistGroupId,
        newToSlice: List<PlaylistAbstract>,
    ) {
        val old = _playlists.value
        val idxInSlice = newToSlice.indexOfFirst { it.meta.id == playlistId }
        if (idxInSlice < 0) {
            return
        }
        // The moved item's meta must reflect the new group — section
        // membership is derived from `meta.groupId`.
        val moved = newToSlice[idxInSlice].let {
            it.copy(meta = it.meta.copy(groupId = toGroupId))
        }
        val sliceWithMove = newToSlice.toMutableList().apply { set(idxInSlice, moved) }

        // Rebuild the flat list per group (groups in group order, each
        // group's slice in its (new) member order).
        val rebuilt = _groups.value.flatMap { group ->
            when (group.id) {
                toGroupId -> sliceWithMove
                else -> old.filter { it.meta.groupId == group.id && it.meta.id != playlistId }
            }
        }
        // Defensive: a dangling group-less playlist (shouldn't happen)
        // keeps its place at the end.
        val known = _groups.value.map { it.id }.toSet()
        val strays = old.filter { it.meta.groupId == null || !known.contains(it.meta.groupId) }
        _playlists.value = (rebuilt + strays).toPersistentList()

        val a = sliceWithMove.getOrNull(idxInSlice - 1)?.meta?.id
        val b = sliceWithMove.getOrNull(idxInSlice + 1)?.meta?.id

        _scope.launch {
            bridge.call(
                BridgeMethods.PlaylistGroup.MOVE_PLAYLIST,
                ArgMovePlaylistToGroup(
                    playlistId = playlistId,
                    groupId = toGroupId,
                    a = a,
                    b = b,
                ),
            ).unwrapOrNull()
            scheduleReload()
        }
    }

    /**
     * Optimistic group reorder: [newOrder] is the group list after the
     * move, [movedId] the dragged group's header. Neighbor anchors are
     * taken from the new order.
     */
    fun applyGroupOrder(newOrder: List<PlaylistGroupMeta>, movedId: PlaylistGroupId) {
        if (newOrder.map { it.id } == _groups.value.map { it.id }) {
            return
        }
        _groups.value = newOrder.toPersistentList()

        val idx = newOrder.indexOfFirst { it.id == movedId }
        if (idx < 0) {
            return
        }
        val a = newOrder.getOrNull(idx - 1)?.id
        val b = newOrder.getOrNull(idx + 1)?.id

        _scope.launch {
            bridge.call(
                BridgeMethods.PlaylistGroup.REORDER,
                ArgReorderPlaylistGroup(id = movedId, a = a, b = b),
            ).unwrapOrNull()
            scheduleReload()
        }
    }

    fun createGroup(title: String) {
        _scope.launch {
            bridge.call(
                BridgeMethods.PlaylistGroup.CREATE,
                ArgCreatePlaylistGroup(title = title),
            ).unwrapOrNull()
            reload()
        }
    }

    fun renameGroup(id: PlaylistGroupId, title: String) {
        _scope.launch {
            bridge.call(
                BridgeMethods.PlaylistGroup.UPDATE,
                ArgUpdatePlaylistGroup(id = id, title = title),
            ).unwrapOrNull()
            reload()
        }
    }

    /**
     * Delete a group. [deletePlaylists] = false sweeps its playlists
     * into the first remaining group; true removes the playlists with
     * it (full cascade). The last group cannot be removed — [onError]
     * receives the bridge error code (e.g. `LastGroupCannotRemove`) so
     * the UI can surface a toast instead of silently no-op-ing.
     *
     * When cascading, every member playlist's removal is pre-announced
     * through [preRemovePlaylistEvent] so playback of the current
     * playlist stops before the rows disappear.
     */
    fun removeGroup(
        id: PlaylistGroupId,
        deletePlaylists: Boolean,
        onError: ((String) -> Unit)? = null,
    ) {
        _scope.launch {
            if (deletePlaylists) {
                for (member in _playlists.value.filter { it.meta.groupId == id }) {
                    _preRemovePlaylistEvent.emit(member.meta.id)
                }
            }
            val ret = bridge.call(
                BridgeMethods.PlaylistGroup.REMOVE,
                ArgRemovePlaylistGroup(id = id, deletePlaylists = deletePlaylists),
            )
            val result = ret.unwrapOrNull()
            if (result == null) {
                val code = ret.errorCode
                if (code != null) {
                    onError?.invoke(code)
                }
            } else {
                reload()
            }
        }
    }

    /** Persist a group's expand/collapse state; optimistic local flip. */
    fun setGroupExpanded(id: PlaylistGroupId, expanded: Boolean) {
        _groups.value = _groups.value
            .map { group ->
                if (group.id == id) group.copy(expanded = expanded) else group
            }
            .toPersistentList()
        _scope.launch {
            bridge.call(
                BridgeMethods.PlaylistGroup.SET_EXPANDED,
                ArgSetPlaylistGroupExpanded(id = id, expanded = expanded),
            ).unwrapOrNull()
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
        // Self-heal first: the Default group must exist (and orphaned
        // playlists be swept into it) before either list is read.
        bridge.call(
            BridgeMethods.PlaylistGroup.ENSURE_DEFAULT,
            ArgEnsurePlaylistGroups(defaultTitle = defaultGroupTitle),
        ).unwrapOrNull()
        val groups: List<PlaylistGroupMeta>? =
            bridge.call(BridgeMethods.PlaylistGroup.LIST).unwrapOrNull()?.payload
        _groups.value = groups?.toPersistentList() ?: persistentListOf()
        val list: List<PlaylistAbstract>? = bridge.call(BridgeMethods.Playlist.LIST).unwrapOrNull()?.payload
        _playlists.value = list?.toPersistentList() ?: persistentListOf()
    }
}
