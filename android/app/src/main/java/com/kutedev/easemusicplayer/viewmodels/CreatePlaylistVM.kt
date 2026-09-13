package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.ImportRepository
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import com.kutedev.easemusicplayer.singleton.types.ArgCreatePlaylist
import com.kutedev.easemusicplayer.singleton.types.CreatePlaylistMode
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupMeta
import com.kutedev.easemusicplayer.singleton.types.Storage
import com.kutedev.easemusicplayer.singleton.types.StorageEntry
import com.kutedev.easemusicplayer.singleton.types.StorageEntryType
import com.kutedev.easemusicplayer.singleton.types.StorageAllowlistMode
import com.kutedev.easemusicplayer.singleton.types.StorageId
import com.kutedev.easemusicplayer.singleton.types.ToAddMusicEntry
import com.kutedev.easemusicplayer.singleton.types.StorageEntryLoc
import java.net.URLDecoder
import javax.inject.Inject
import kotlin.collections.firstOrNull
import kotlin.collections.map

@HiltViewModel
class CreatePlaylistVM @Inject constructor(
    private val importRepository: ImportRepository,
    private val playlistRepository: PlaylistRepository,
    private val storageRepository: StorageRepository
) : ViewModel() {
    private val _modalOpen = MutableStateFlow(false)
    private val _mode = MutableStateFlow(CreatePlaylistMode.FULL)
    private val _fullImported = MutableStateFlow(false)
    private val _entries = MutableStateFlow(listOf<StorageEntry>())
    private val _name = MutableStateFlow("")
    private val _cover = MutableStateFlow<StorageEntryLoc?>(null)
    private val _advancedOpen = MutableStateFlow(false)
    private val _allowlistMode = MutableStateFlow(StorageAllowlistMode.ALL)
    private val _allowlistStorages = MutableStateFlow(listOf<StorageId>())
    private val _groupId = MutableStateFlow<com.kutedev.easemusicplayer.singleton.types.PlaylistGroupId?>(null)
    val mode = _mode.asStateFlow()
    val musicCount = _entries.map { entries ->
        entries.count { entry ->  entry.entryTyp() == StorageEntryType.MUSIC }
    }.stateIn(viewModelScope, SharingStarted.Lazily, 0)
    val name = _name.asStateFlow()
    val recommendPlaylistNames = _entries.map { entries ->
        var l = mutableListOf<String>()
        var set = HashSet<String>()

        for (entry in entries) {
            for (p in entry.path.split("/").let { list -> if (list.size == 0) emptyList() else list.take(list.size - 1) }) {
                if (p.isNotBlank()) {
                    val x = try {
                        URLDecoder.decode(p.trim(), "UTF-8")
                    } catch (e: Exception) {
                        p.trim()
                    }

                    if (!set.contains(x)) {
                        set.add(x)
                        l.add(x)
                    }
                }
            }
        }

        l.takeLast(6)
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Lazily,
        initialValue = emptyList()
    )

    val cover = _cover.asStateFlow()
    val modalOpen = _modalOpen.asStateFlow()
    val fullImported = _fullImported.asStateFlow()

    val storages: StateFlow<List<Storage>> = storageRepository.storages
    val groups: StateFlow<List<PlaylistGroupMeta>> = playlistRepository.groups
    val groupId = _groupId.asStateFlow()
    val advancedOpen = _advancedOpen.asStateFlow()
    val allowlistMode = _allowlistMode.asStateFlow()
    val allowlistStorages = _allowlistStorages.asStateFlow()

    /**
     * Storages the imported entries live on — in SPECIFIC mode these can
     * never be unchecked (the playlist-to-be must keep importing from the
     * storages its musics come from). Only bites when the import ran
     * before the allowlist was narrowed; the reverse order can't produce
     * a violation because the import picker is already restricted.
     */
    val lockedAllowlistStorages = _entries.map { entries ->
        entries.map { entry -> entry.storageId }.distinct()
    }.stateIn(viewModelScope, SharingStarted.Lazily, listOf())

    /** Selection as persisted: user picks ∪ locked (locked are checked). */
    private fun effectiveAllowlistStorages(): List<StorageId> {
        return (_allowlistStorages.value + lockedAllowlistStorages.value).distinct()
    }

    /** Restriction handed to the import picker; null = unrestricted. An
     *  empty SPECIFIC selection stays empty (nothing importable) rather
     *  than silently falling back to unrestricted — imports from wrong
     *  storages would become locked afterwards. */
    private fun currentAllowlist(): List<StorageId>? {
        if (_allowlistMode.value != StorageAllowlistMode.SPECIFIC) {
            return null
        }
        return effectiveAllowlistStorages()
    }

    val canSubmit = combine(
        name,
        mode,
        musicCount,
        cover,
        combine(_allowlistMode, _allowlistStorages, _groupId) { m, s, g -> Triple(m, s, g) }
    ) {
            name, mode, musicCount, cover, (allowlistMode, allowlistStorages, groupId) ->
        val allowlistOk = allowlistMode != StorageAllowlistMode.SPECIFIC ||
            (allowlistStorages + lockedAllowlistStorages.value).distinct().isNotEmpty()
        val groupOk = groupId != null
        if (mode == CreatePlaylistMode.FULL) {
             name.isNotBlank() && (musicCount > 0 || cover != null) && allowlistOk && groupOk
        } else {
            name.isNotBlank() && allowlistOk && groupOk
        }
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Lazily,
        initialValue = false
    )

    fun updateName(name: String) {
        _name.value = name
    }

    fun clearCover() {
        _cover.value = null
    }

    fun updateMode(mode: CreatePlaylistMode) {
        _mode.value = mode
    }

    fun toggleAdvanced() {
        _advancedOpen.value = !_advancedOpen.value
    }

    fun updateAllowlistMode(mode: StorageAllowlistMode) {
        _allowlistMode.value = mode
        if (mode == StorageAllowlistMode.SPECIFIC) {
            // Locked storages are always part of the checked set.
            _allowlistStorages.value = effectiveAllowlistStorages()
        }
    }

    fun toggleAllowlistStorage(id: StorageId) {
        if (lockedAllowlistStorages.value.contains(id)) {
            return
        }
        val current = _allowlistStorages.value
        _allowlistStorages.value = if (current.contains(id)) {
            current - id
        } else {
            current + id
        }
    }

    fun updateGroupId(id: com.kutedev.easemusicplayer.singleton.types.PlaylistGroupId) {
        _groupId.value = id
    }

    /**
     * Open the create dialog. [defaultGroupId] preselects the group
     * (e.g. the first group when opened from the empty state or the
     * toolbar `+`); null picks the first group once loaded.
     */
    fun openModal(defaultGroupId: com.kutedev.easemusicplayer.singleton.types.PlaylistGroupId? = null) {
        _groupId.value = defaultGroupId ?: playlistRepository.groups.value.firstOrNull()?.id
        _modalOpen.value = true
    }

    fun closeModal() {
        _modalOpen.value = false

        reset()
    }

    fun reset() {
        _mode.value = CreatePlaylistMode.FULL
        _fullImported.value = false
        _name.value = ""
        _cover.value = null
        _advancedOpen.value = false
        _allowlistMode.value = StorageAllowlistMode.ALL
        _allowlistStorages.value = listOf()
        _groupId.value = null
    }

    fun prepareImportCreate() {
        importRepository.prepare(
            listOf(StorageEntryType.MUSIC, StorageEntryType.IMAGE),
            currentAllowlist()
        ) {
                entries ->
            _entries.value = entries.filter { v -> v.entryTyp() == StorageEntryType.MUSIC }
            _cover.value = entries.filter { v -> v.entryTyp() == StorageEntryType.IMAGE }.map { v ->
                StorageEntryLoc(v.storageId, v.path) }.firstOrNull()
            _fullImported.value = true

            // Late-arriving referenced storages must stay checked in
            // SPECIFIC mode.
            if (_allowlistMode.value == StorageAllowlistMode.SPECIFIC) {
                _allowlistStorages.value = effectiveAllowlistStorages()
            }

            val name = recommendPlaylistNames.value.lastOrNull()
            if (name != null) {
                _name.value = name
            }
        }
    }

    fun finish() {
        // Only the FULL tab carries the pending import; the EMPTY tab
        // must create a playlist with no musics even if an import was
        // picked earlier on the FULL tab (the pending import survives
        // tab toggling — RESET is what discards it).
        val entries = if (_mode.value == CreatePlaylistMode.FULL) {
            _entries.value.map { entry -> ToAddMusicEntry(entry, entry.name) }
        } else {
            emptyList()
        }
        val groupId = _groupId.value ?: return

        playlistRepository.createPlaylist(ArgCreatePlaylist(
            title = _name.value,
            cover = _cover.value,
            entries = entries,
            storageAllowlist = if (_allowlistMode.value == StorageAllowlistMode.SPECIFIC) {
                effectiveAllowlistStorages()
            } else {
                null
            },
            groupId = groupId,
        ))
    }
}
