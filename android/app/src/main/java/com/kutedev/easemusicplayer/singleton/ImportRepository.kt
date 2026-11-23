package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.StorageEntry
import com.kutedev.easemusicplayer.singleton.types.StorageEntryLoc
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
class ImportRepository @Inject constructor(
    private val bridge: Bridge,
    private val scope: CoroutineScope,
) {
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

    /**
     * Folder (storage + path) of the most recent completed import, so the
     * Import page can reopen there. `null` when nothing was imported yet
     * (or the read fails — the bridge logs it).
     */
    suspend fun loadLastImportLoc(): StorageEntryLoc? =
        bridge.call(BridgeMethods.Preference.GET_LAST_IMPORT_LOC)
            .unwrapOrNull()
            ?.payload

    /**
     * Persist [loc] as the last import folder, fire-and-forget on the
     * repository scope: the caller ([ImportVM.finish]) runs right after
     * the page popped the route, so the save must outlive the VM.
     */
    fun saveLastImportLoc(loc: StorageEntryLoc) {
        scope.launch {
            bridge.call(BridgeMethods.Preference.SAVE_LAST_IMPORT_LOC, loc).unwrapOrNull()
        }
    }
}
