package com.kutedev.easemusicplayer.repositories

import com.kutedev.easemusicplayer.core.Bridge
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.StorageEntry
import uniffi.ease_client_backend.StorageEntryType
import javax.inject.Inject

typealias ImportHandler = (entries: List<StorageEntry>) -> Unit


object RouteImportType {
    val Music = "Music"
    val Lyric = "Lyric"
    val EditPlaylist = "EditPlaylist"
    val EditPlaylistCover = "EditPlaylistCover"
}

class ImportRepository @Inject constructor(
    private val bridge: Bridge,
    private val storageRepository: StorageRepository,
    private val scope: CoroutineScope
) {
    private val _allowTypes = MutableStateFlow(listOf<StorageEntryType>())
    private var _importCallback: ((List<StorageEntry>) -> Unit)? = null

    val allowTypes = _allowTypes.asStateFlow()

    fun prepare(types: List<StorageEntryType>, block: ImportHandler) {
        _allowTypes.value = types
        _importCallback = block
    }

    fun onFinish(entries: List<StorageEntry>) {
        val c = _importCallback
        if (c != null) {
            c(entries)
        }
    }
}
