package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import com.kutedev.easemusicplayer.singleton.types.StorageEntry
import com.kutedev.easemusicplayer.singleton.types.StorageEntryType
import com.kutedev.easemusicplayer.singleton.types.StorageId
import javax.inject.Inject
import javax.inject.Singleton

typealias ImportHandler = (entries: List<StorageEntry>) -> Unit


object RouteImportType {
    val Music = "Music"
    val Lyric = "Lyric"
    val EditPlaylist = "EditPlaylist"
    val EditPlaylistCover = "EditPlaylistCover"
}

@Singleton
class ImportRepository @Inject constructor() {
    private val _allowTypes = MutableStateFlow(listOf<StorageEntryType>())
    private val _allowedStorageIds = MutableStateFlow<List<StorageId>?>(null)
    private var _importCallback: ((List<StorageEntry>) -> Unit)? = null

    val allowTypes = _allowTypes.asStateFlow()

    /**
     * Import-source restriction for the prepared session: `null` = every
     * storage is offered in the Import page's picker; a list = only those
     * storages (playlist storage-allowlist). Reset by every [prepare].
     */
    val allowedStorageIds = _allowedStorageIds.asStateFlow()

    fun prepare(
        types: List<StorageEntryType>,
        allowStorageIds: List<StorageId>? = null,
        block: ImportHandler
    ) {
        _allowTypes.value = types
        _allowedStorageIds.value = allowStorageIds
        _importCallback = block
    }

    fun onFinish(entries: List<StorageEntry>) {
        val c = _importCallback
        _importCallback = null
        if (c != null) {
            c(entries)
        }
    }
}
