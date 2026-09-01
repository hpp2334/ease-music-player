package com.kutedev.easemusicplayer.viewmodels

import android.content.Context
import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.BridgeMethods
import com.kutedev.easemusicplayer.singleton.ImportRepository
import com.kutedev.easemusicplayer.singleton.PermissionRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import com.kutedev.easemusicplayer.singleton.ToastRepository
import com.kutedev.easemusicplayer.utils.SafUri
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.collections.immutable.persistentHashSetOf
import kotlinx.collections.immutable.persistentListOf
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.CurrentStorageStateType
import com.kutedev.easemusicplayer.singleton.types.ListStorageEntryChildrenResp
import com.kutedev.easemusicplayer.singleton.types.Storage
import com.kutedev.easemusicplayer.singleton.types.StorageEntry
import com.kutedev.easemusicplayer.singleton.types.StorageEntryLoc
import com.kutedev.easemusicplayer.singleton.types.StorageEntryType
import com.kutedev.easemusicplayer.singleton.types.StorageHandle
import com.kutedev.easemusicplayer.singleton.types.StorageId
import java.net.URLDecoder

data class SplitPathItem(
    val path: String,
    val name: String,
)

private fun defaultSplitPaths(): List<SplitPathItem> {
    return listOf()
}

@HiltViewModel
class ImportVM @Inject constructor(
    private val storageRepository: StorageRepository,
    private val importRepository: ImportRepository,
    private val permissionRepository: PermissionRepository,
    private val toastRepository: ToastRepository,
    @ApplicationContext private val appContext: Context,
    private val bridge: Bridge
) : ViewModel() {
    private val _currentPath = MutableStateFlow("/")
    private val _splitPaths = _currentPath.map { path ->
        val components = path.split('/').filter { it.isNotEmpty() }
        val splitPaths = mutableListOf<SplitPathItem>()

        var currentPath = ""
        for (component in components) {
            currentPath = if (currentPath == "/") {
                "/$component"
            } else {
                "$currentPath/$component"
            }
            val name = try {
                URLDecoder.decode(component, "UTF-8")
            } catch (e: Exception) {
                component
            }
            splitPaths.add(SplitPathItem(currentPath, name))
        }

        splitPaths
    }.stateIn(viewModelScope, SharingStarted.Lazily, defaultSplitPaths())
    private val _selected = MutableStateFlow(persistentHashSetOf<String>())
    private val _entries = MutableStateFlow(listOf<StorageEntry>())
    private val _selectedStorageId = MutableStateFlow(storageRepository.storages.value.firstOrNull()?.id)
    private val _loadState = MutableStateFlow(CurrentStorageStateType.LOADING)
    private val _disabledToggleAll = _entries.map { entries ->
        entries.all { it.isDir }
    }.stateIn(viewModelScope, SharingStarted.Lazily, true)
    private val _undoStack = MutableStateFlow(persistentListOf<String>())

    val splitPaths = _splitPaths
    val selectedCount = _selected.combine(_entries) { selected, entries ->
        entries.count { entry -> selected.contains(entry.path) }
    }.stateIn(viewModelScope, SharingStarted.Lazily, 0)
    val entries = _entries.asStateFlow()
    val selected = _selected.asStateFlow()
    val allowTypes = importRepository.allowTypes
    val selectedStorageId = _selectedStorageId.asStateFlow()
    val loadState = _loadState.asStateFlow()
    val isLocalSelected = combine(_selectedStorageId, storageRepository.storages) { selectedId, storages ->
        val storage = storages.find { storage -> storage.id == selectedId }
        storage?.handle is StorageHandle.Local
    }.stateIn(
        viewModelScope,
        SharingStarted.Lazily,
        storageRepository.storages.value.find { storage -> storage.id == _selectedStorageId.value }
            ?.handle is StorageHandle.Local
    )
    val disabledToggleAll = _disabledToggleAll
    val canUndo =
        _undoStack.map {
            undoStack -> undoStack.isNotEmpty()
        }.stateIn(viewModelScope, SharingStarted.Lazily, false)


    init {
        viewModelScope.launch {
            storageRepository.storages.collect { storages ->
                val storage = storages.find { storage -> storage.id == _selectedStorageId.value }
                if (storage == null) {
                    _selectedStorageId.value = storageRepository.storages.value.firstOrNull()?.id
                }

                reload()
            }
        }
        viewModelScope.launch {
            reload()
        }
        viewModelScope.launch {
            permissionRepository.havePermission.collect {
                reload()
            }
        }
    }

    fun clickEntry(entry: StorageEntry) {
        if (entry.isDir) {
            navigateDir(entry.path)
        } else if (allowTypes.value.contains(entry.entryTyp())) {
            toggleSelect(entry.path)
        }
    }

    fun navigateDir(path: String) {
        pushCurrentToUndoStack()
        navigateDirImpl(path)
    }

    private fun toggleSelect(path: String) {
        val selected = _selected.value
        val next = {
            if (selected.contains(path)) {
                selected.remove(path)
            } else {
                selected.add(path)
            }
        }()
        _selected.value = next
    }

    fun finish() {
        val v = _entries.value.filter { entry -> _selected.value.contains(entry.path) }
        importRepository.onFinish(v)
    }

    fun requestPermission() {
        permissionRepository.requestStoragePermission()
    }

    /**
     * MIME types for the native system file picker, derived from the prepared
     * import types. `OpenMultipleDocuments` passes them as `EXTRA_MIME_TYPES`.
     */
    fun allowMimeTypes(): List<String> {
        val types = allowTypes.value
        val mimes = mutableListOf<String>()
        if (types.contains(StorageEntryType.MUSIC)) {
            mimes.add("audio/*")
        }
        if (types.contains(StorageEntryType.IMAGE)) {
            mimes.add("image/*")
        }
        if (mimes.isEmpty() || types.contains(StorageEntryType.LYRIC)) {
            // `.lrc` has no stable MIME type — accept everything and rely on
            // the extension filter in [onPickedUris].
            return listOf("*/*")
        }
        return mimes
    }

    /**
     * Handle results of the native system file picker for the Local storage:
     * resolve each URI to a real path under /storage/emulated/0, map it to a
     * [StorageEntry] and hand the batch to the prepared import callback.
     * Unresolvable or type-mismatched picks are skipped with a toast.
     *
     * @return true when the import finished and the caller should leave the page.
     */
    fun onPickedUris(uris: List<Uri>): Boolean {
        if (uris.isEmpty()) {
            return false
        }
        val storage = currentStorage() ?: return false
        if (storage.handle !is StorageHandle.Local) {
            return false
        }

        val entries = mutableListOf<StorageEntry>()
        var skipped = 0
        for (uri in uris) {
            val path = SafUri.resolveLocalPath(appContext, uri)
            if (path == null) {
                skipped += 1
                continue
            }
            val entry = StorageEntry(
                storageId = storage.id,
                name = SafUri.queryDisplayName(appContext, uri) ?: path.substringAfterLast('/'),
                path = path,
                size = SafUri.querySize(appContext, uri),
                isDir = false,
            )
            if (allowTypes.value.contains(entry.entryTyp())) {
                entries.add(entry)
            } else {
                skipped += 1
            }
        }
        if (skipped > 0) {
            toastRepository.emitToastRes(R.string.import_local_skipped)
        }
        if (entries.isEmpty()) {
            return false
        }
        importRepository.onFinish(entries)
        return true
    }

    fun selectStorage(storageId: StorageId) {
        _selectedStorageId.value = storageId
        _undoStack.value = persistentListOf()

        navigateDirImpl("/")
    }

    fun toggleAll() {
        val allSelected = _selected.value.size == _entries.value.size
        if (allSelected) {
            _selected.update { selected ->
                selected.clear()
            }
        } else {
            _selected.update { selected ->
                selected.clear().addAll(_entries.value.map { it.path })
            }
        }
    }

    fun reload() {
        val storage = currentStorage() ?: return

        if (storage.handle is StorageHandle.Local) {
            // Local files are picked through the native system file picker —
            // there is nothing to browse. Reading the picked files (and
            // playing them) still requires the storage permission.
            if (!permissionRepository.havePermission.value) {
                _loadState.value = CurrentStorageStateType.NEED_PERMISSION
            } else {
                _entries.value = emptyList()
                _loadState.value = CurrentStorageStateType.OK
            }
            return
        }

        _loadState.value = CurrentStorageStateType.LOADING
        _entries.value = emptyList()

        viewModelScope.launch {
            val loc = StorageEntryLoc(
                storageId = storage.id,
                path = currentPath(),
            )
            val resp: ListStorageEntryChildrenResp? = try {
                bridge.call(BridgeMethods.Storage.LIST_ENTRY_CHILDREN, loc).unwrapOrThrow().payload
            } catch (e: Throwable) {
                null
            }

            when (resp) {
                null -> {
                    _loadState.value = CurrentStorageStateType.UNKNOWN_ERROR
                }
                is ListStorageEntryChildrenResp.Ok -> {
                    _loadState.value = CurrentStorageStateType.OK
                    _entries.value = resp.data
                }

                ListStorageEntryChildrenResp.AuthenticationFailed -> {
                    _loadState.value = CurrentStorageStateType.AUTHENTICATION_FAILED
                }

                ListStorageEntryChildrenResp.Timeout -> {
                    _loadState.value = CurrentStorageStateType.TIMEOUT
                }

                ListStorageEntryChildrenResp.Unknown -> {
                    _loadState.value = CurrentStorageStateType.UNKNOWN_ERROR
                }
            }
        }
    }

    fun undo() {
        val current = popCurrentFromUndoStack()
        if (current != null) {
            navigateDirImpl(current)
        }
    }

    private fun currentPath(): String {
        val p = _splitPaths.value.lastOrNull()?.path

        if (p == null) {
            return "/"
        }
        return p
    }

    private fun currentStorage(): Storage? {
        val storage = storageRepository.storages.value.find { storage -> storage.id == _selectedStorageId.value }
        return storage
    }

    private fun pushCurrentToUndoStack() {
        val currentUndoStack = _undoStack.value
        val nextUndoStack = currentUndoStack.add(currentPath())
        _undoStack.value = nextUndoStack
    }

    private fun popCurrentFromUndoStack(): String? {
        val currentUndoStack = _undoStack.value
        val current = currentUndoStack.lastOrNull()
        if (current != null) {
            val next = currentUndoStack.removeAt(currentUndoStack.lastIndex)
            _undoStack.value = next
        }
        return current
    }


    private fun navigateDirImpl(path: String) {
        _currentPath.value = path
        _selected.update { selected ->
            selected.clear()
        }

        reload()
    }
}

class VImportStorageEntry(private val storage: Storage) {
    val id: StorageId
        get() = storage.id

    val isLocal: Boolean
        get() = storage.handle is StorageHandle.Local

    val name: String
        get() = storage.alias

    /** Provider name for plugin storages (e.g. "webdav" / "onedrive"). */
    val subtitle: String
        get() = when (val handle = storage.handle) {
            is StorageHandle.Plugin -> handle.pluginStorageId.id.substringBefore(':')
            else -> ""
        }
}
