package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.ToastRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import com.kutedev.easemusicplayer.singleton.types.PlaylistAbstract
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupId
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupMeta
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import javax.inject.Inject

data class PlaylistsState(
    val playlists: List<PlaylistAbstract> = listOf()
)

enum class PlaylistsMode {
    Normal,
    Adjust
}

/** One expandable section of the playlists grid: a group + its playlists. */
data class PlaylistGroupSection(
    val group: PlaylistGroupMeta,
    val playlists: List<PlaylistAbstract>,
)

/**
 * Flat grid-item model: full-span headers interleaved with playlist
 * cards. Collapsed groups contribute only their header. The keys ride
 * the LazyVerticalGrid `key` contract (stable ids across recomposition).
 */
sealed interface PlaylistGridEntry {
    val key: String

    data class Header(val group: PlaylistGroupMeta) : PlaylistGridEntry {
        override val key: String = "group_${group.id.value}"
    }

    data class Card(
        val group: PlaylistGroupMeta,
        val playlist: PlaylistAbstract,
    ) : PlaylistGridEntry {
        override val key: String = "playlist_${playlist.meta.id.value}"
    }
}

/** Build the flat grid-item list from the sections (collapsed = header only). */
fun buildPlaylistGridEntries(sections: List<PlaylistGroupSection>): List<PlaylistGridEntry> =
    buildList {
        for (section in sections) {
            add(PlaylistGridEntry.Header(section.group))
            if (section.group.expanded) {
                for (playlist in section.playlists) {
                    add(PlaylistGridEntry.Card(section.group, playlist))
                }
            }
        }
    }

@HiltViewModel
class PlaylistsVM @Inject constructor(
    private val playlistRepository: PlaylistRepository,
    private val toastRepository: ToastRepository,
) : ViewModel() {
    private val _mode = MutableStateFlow(PlaylistsMode.Normal)
    val playlists = playlistRepository.playlists
    val groups = playlistRepository.groups

    val mode = _mode.asStateFlow()

    /**
     * Groups joined with their playlists (global order-key order,
     * filtered by `groupId`). Orphaned playlists (NULL or dangling
     * group — only possible transiently) render under the first group
     * so nothing is ever hidden.
     */
    val sections: StateFlow<List<PlaylistGroupSection>> =
        combine(playlistRepository.groups, playlistRepository.playlists) { groups, playlists ->
            if (groups.isEmpty()) {
                emptyList()
            } else {
                groups.mapIndexed { index, group ->
                    val mine = playlists.filter { it.meta.groupId == group.id }
                    if (index == 0) {
                        val orphans = playlists.filter { p ->
                            val gid = p.meta.groupId
                            gid == null || groups.none { it.id == gid }
                        }
                        PlaylistGroupSection(group, mine + orphans)
                    } else {
                        PlaylistGroupSection(group, mine)
                    }
                }
            }
        }.stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(5000),
            initialValue = emptyList(),
        )

    fun setMode(mode: PlaylistsMode) {
        _mode.value = mode
    }

    fun toggleMode() {
        _mode.value = when (_mode.value) {
            PlaylistsMode.Normal -> PlaylistsMode.Adjust
            PlaylistsMode.Adjust -> PlaylistsMode.Normal
        }
    }

    /**
     * Toggle by id (NOT by the captured group object): the header's
     * `pointerInput` block outlives recompositions, so a captured
     * `PlaylistGroupMeta` goes stale after the first toggle — reading
     * the current state here is immune to that.
     */
    fun toggleExpanded(groupId: PlaylistGroupId) {
        val current = playlistRepository.groups.value.firstOrNull { it.id == groupId } ?: return
        playlistRepository.setGroupExpanded(groupId, !current.expanded)
    }

    fun createGroup(title: String) {
        playlistRepository.createGroup(title)
    }

    fun renameGroup(group: PlaylistGroupMeta, title: String) {
        playlistRepository.renameGroup(group.id, title)
    }

    fun removeGroup(group: PlaylistGroupMeta, deletePlaylists: Boolean) {
        playlistRepository.removeGroup(group.id, deletePlaylists) { code ->
            if (code == "LastGroupCannotRemove") {
                toastRepository.emitToastRes(R.string.playlist_group_last_delete_error)
            } else {
                toastRepository.emitToast(code)
            }
        }
    }

    /**
     * Single drag-drop commit for a playlist card (the custom drag
     * controller resolves [slot] from pointer geometry; nothing is
     * mutated during the gesture).
     *
     * [slot] is the insert index with `removeAt(origin).add(slot)`
     * semantics for a within-group move (the slot was resolved over the
     * slice that still contains the dragged card as a placeholder), or
     * the plain insert index among the target's current members for a
     * cross-group move.
     */
    fun commitCardMove(
        playlistId: PlaylistId,
        originGroupId: PlaylistGroupId,
        targetGroupId: PlaylistGroupId,
        slot: Int,
    ) {
        val origin = sections.value.firstOrNull { it.group.id == originGroupId }
        val target = sections.value.firstOrNull { it.group.id == targetGroupId }
        val item = origin?.playlists?.firstOrNull { it.meta.id == playlistId }
        if (origin == null || target == null || item == null) {
            return
        }

        if (originGroupId == targetGroupId) {
            val originSlot = origin.playlists.indexOfFirst { it.meta.id == playlistId }
            if (originSlot < 0) {
                return
            }
            val slice = origin.playlists.toMutableList()
            slice.removeAt(originSlot)
            // The resolved slot counts over the slice that still
            // contains the dragged card, so a downward target is one
            // past where it should land after the removal — adjust by
            // one to make "lands before the indicator's outlined card"
            // exact (dropping onto the boundary right of the next
            // neighbor is then a no-op, not a swap).
            val insertAt = (
                if (slot > originSlot) {
                    slot - 1
                } else {
                    slot
                }
                ).coerceIn(0, slice.size)
            slice.add(insertAt, item)
            if (slice.map { it.meta.id } == origin.playlists.map { it.meta.id }) {
                return
            }
            playlistRepository.applyGroupPlaylistOrder(targetGroupId, slice, playlistId)
        } else {
            val slice = target.playlists.toMutableList()
            slice.add(slot.coerceIn(0, slice.size), item)
            playlistRepository.applyPlaylistGroupMove(playlistId, targetGroupId, slice)
        }
    }

    /**
     * Single drag-drop commit for a group header: move the dragged
     * group right before [beforeGroupId] (null = to the end).
     */
    fun commitGroupMove(draggedId: PlaylistGroupId, beforeGroupId: PlaylistGroupId?) {
        val order = sections.value.map { it.group }.toMutableList()
        val index = order.indexOfFirst { it.id == draggedId }
        if (index < 0) {
            return
        }
        val dragged = order.removeAt(index)
        val at = beforeGroupId
            ?.let { id -> order.indexOfFirst { it.id == id } }
            ?: order.size
        if (at < 0) {
            return
        }
        order.add(at, dragged)
        if (order.map { it.id } == sections.value.map { it.group.id }) {
            return
        }
        playlistRepository.applyGroupOrder(order, draggedId)
    }
}
