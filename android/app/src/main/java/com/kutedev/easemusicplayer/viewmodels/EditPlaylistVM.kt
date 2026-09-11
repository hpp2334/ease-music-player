package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.SavedStateHandle
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
import kotlinx.coroutines.flow.stateIn
import com.kutedev.easemusicplayer.singleton.types.ArgUpdatePlaylist
import com.kutedev.easemusicplayer.singleton.types.Storage
import com.kutedev.easemusicplayer.singleton.types.StorageAllowlistMode
import com.kutedev.easemusicplayer.singleton.types.StorageEntryType
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import com.kutedev.easemusicplayer.singleton.types.StorageId
import com.kutedev.easemusicplayer.singleton.types.StorageEntryLoc
import javax.inject.Inject
import kotlin.collections.firstOrNull
import kotlin.collections.map

@HiltViewModel
class EditPlaylistVM @Inject constructor(
    private val importRepository: ImportRepository,
    private val playlistRepository: PlaylistRepository,
    private val storageRepository: StorageRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {
    private val _id: PlaylistId = PlaylistId(savedStateHandle["id"]!!)
    private val _modalOpen = MutableStateFlow(false)
    private val _name = MutableStateFlow("")
    private val _cover = MutableStateFlow<StorageEntryLoc?>(null)
    private val _advancedOpen = MutableStateFlow(false)
    private val _allowlistMode = MutableStateFlow(StorageAllowlistMode.ALL)
    private val _allowlistStorages = MutableStateFlow(listOf<StorageId>())
    private val _lockedAllowlistStorages = MutableStateFlow(listOf<StorageId>())
    val name = _name.asStateFlow()
    val cover = _cover.asStateFlow()
    val modalOpen = _modalOpen.asStateFlow()

    val storages: StateFlow<List<Storage>> = storageRepository.storages
    val advancedOpen = _advancedOpen.asStateFlow()
    val allowlistMode = _allowlistMode.asStateFlow()
    val allowlistStorages = _allowlistStorages.asStateFlow()

    /**
     * Storages the playlist's musics live on (`PlaylistAbstract
     * .musicStorageIds`, loaded at [openModal]) — in SPECIFIC mode these
     * can never be unchecked and are always part of the saved allowlist.
     */
    val lockedAllowlistStorages = _lockedAllowlistStorages.asStateFlow()

    /** Selection as persisted: user picks ∪ locked (locked are checked). */
    private fun effectiveAllowlistStorages(): List<StorageId> {
        return (_allowlistStorages.value + _lockedAllowlistStorages.value).distinct()
    }

    /** Restriction handed to the import picker; null = unrestricted. */
    private fun currentAllowlist(): List<StorageId>? {
        if (_allowlistMode.value != StorageAllowlistMode.SPECIFIC) {
            return null
        }
        return effectiveAllowlistStorages()
    }

    val canSubmit = combine(name, _allowlistMode, _allowlistStorages, _lockedAllowlistStorages) {
            name, allowlistMode, allowlistStorages, lockedStorages ->
        val allowlistOk = allowlistMode != StorageAllowlistMode.SPECIFIC ||
            (allowlistStorages + lockedStorages).distinct().isNotEmpty()
        name.isNotBlank() && allowlistOk
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
        if (_lockedAllowlistStorages.value.contains(id)) {
            return
        }
        val current = _allowlistStorages.value
        _allowlistStorages.value = if (current.contains(id)) {
            current - id
        } else {
            current + id
        }
    }

    fun openModal() {
        _modalOpen.value = true


        val list = playlistRepository.playlists.value
        val item = list.find { v -> v.meta.id == _id }

        if (item != null) {
            _name.value = item.meta.title
            _cover.value = item.meta.cover

            val persisted = item.meta.storageAllowlist
            val locked = item.musicStorageIds
            _lockedAllowlistStorages.value = locked

            if (persisted == null) {
                _allowlistMode.value = StorageAllowlistMode.ALL
                _allowlistStorages.value = listOf()
            } else {
                // Only self-heal a stale restriction when we're sure the
                // storage list is loaded (an empty list during startup
                // must not read as "every allowed storage was removed"):
                // neither the persisted ids nor the locked ids resolve to
                // a live storage → fall back to All, so a plain rename is
                // never blocked by a dead allowlist.
                val existing = storageRepository.storages.value
                val live = { ids: List<StorageId> ->
                    if (existing.isEmpty()) {
                        ids
                    } else {
                        ids.filter { id -> existing.any { storage -> storage.id == id } }
                    }
                }
                if (existing.isNotEmpty() && live(persisted).isEmpty() && live(locked).isEmpty()) {
                    _allowlistMode.value = StorageAllowlistMode.ALL
                    _allowlistStorages.value = listOf()
                } else {
                    _allowlistMode.value = StorageAllowlistMode.SPECIFIC
                    // Referenced storages are force-checked even if a
                    // narrower allowlist was saved earlier.
                    _allowlistStorages.value = (persisted + locked).distinct()
                }
            }
        }
    }

    fun closeModal() {
        _modalOpen.value = false

        reset()
    }

    fun reset() {
        _name.value = ""
        _cover.value = null
        _advancedOpen.value = false
        _allowlistMode.value = StorageAllowlistMode.ALL
        _allowlistStorages.value = listOf()
        _lockedAllowlistStorages.value = listOf()
    }

    fun prepareImportCover() {
        importRepository.prepare(listOf(StorageEntryType.IMAGE), currentAllowlist()) {
            entries ->
                _cover.value = entries.map { entry -> StorageEntryLoc(
                    storageId = entry.storageId,
                    path = entry.path
                ) }.firstOrNull()
        }
    }

    fun finish() {
        playlistRepository.editPlaylist(ArgUpdatePlaylist(
            id = _id,
            title = _name.value,
            cover = _cover.value,
            storageAllowlist = if (_allowlistMode.value == StorageAllowlistMode.SPECIFIC) {
                effectiveAllowlistStorages()
            } else {
                null
            },
        ))

        closeModal()
    }
}
