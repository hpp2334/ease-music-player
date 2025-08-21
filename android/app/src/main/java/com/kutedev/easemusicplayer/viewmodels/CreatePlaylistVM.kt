package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.repositories.ImportRepository
import com.kutedev.easemusicplayer.repositories.PlaylistRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.ease_client_backend.ArgCreatePlaylist
import uniffi.ease_client_backend.CreatePlaylistMode
import uniffi.ease_client_backend.StorageEntry
import uniffi.ease_client_backend.StorageEntryType
import uniffi.ease_client_backend.ToAddMusicEntry
import uniffi.ease_client_schema.StorageEntryLoc
import java.net.URLDecoder
import javax.inject.Inject
import kotlin.collections.firstOrNull
import kotlin.collections.map

@HiltViewModel
class CreatePlaylistVM @Inject constructor(
    private val importRepository: ImportRepository,
    private val playlistRepository: PlaylistRepository
) : ViewModel() {
    private val _modalOpen = MutableStateFlow(false)
    private val _mode = MutableStateFlow(CreatePlaylistMode.FULL)
    private val _fullImported = MutableStateFlow(false)
    private val _entries = MutableStateFlow(listOf<StorageEntry>())
    private val _name = MutableStateFlow("")
    private val _cover = MutableStateFlow<StorageEntryLoc?>(null)
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
                    val x = URLDecoder.decode(p.trim(), "UTF-8");
                    
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

    val canSubmit = combine(name, mode, musicCount, cover) {
            name, mode, musicCount, cover ->
        if (mode == CreatePlaylistMode.FULL) {
             name.isNotBlank() && (musicCount > 0 || cover != null)
        } else {
            name.isNotBlank()
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

    fun openModal() {
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
    }

    fun prepareImportCreate() {
        importRepository.prepare(listOf(StorageEntryType.MUSIC, StorageEntryType.IMAGE)) {
                entries ->
            _entries.value = entries.filter { v -> v.entryTyp() == StorageEntryType.MUSIC }
            _cover.value = entries.filter { v -> v.entryTyp() == StorageEntryType.IMAGE }.map { v ->
                StorageEntryLoc(v.storageId, v.path) }.firstOrNull()
            _fullImported.value = true

            val name = recommendPlaylistNames.value.firstOrNull()
            if (name != null) {
                _name.value = name
            }
        }
    }

    fun finish() {
        val entries = _entries.value.map { entry -> ToAddMusicEntry(entry, entry.name) }

        playlistRepository.createPlaylist(ArgCreatePlaylist(
            title = _name.value,
            cover = _cover.value,
            entries = entries
        ))
    }
}
