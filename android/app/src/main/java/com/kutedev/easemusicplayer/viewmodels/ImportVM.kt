package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.BridgeMethods
import com.kutedev.easemusicplayer.singleton.ImportRepository
import com.kutedev.easemusicplayer.singleton.PermissionRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
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
import dagger.hilt.android.lifecycle.HiltViewModel
import java.net.URLDecoder

data class SplitPathItem(
    val path: String,
    val name: String,
)

/** Sort key offered by the import page's sort dialog. */
enum class ImportSortField {
    /** Entry name, case-insensitive. */
    NAME,

    /** Storage-reported creation time (`createdAt`; unknowns sort last). */
    CREATED,

    /** Storage-reported last-modified time (`modifiedAt`; unknowns last). */
    MODIFIED,
}

enum class ImportSortDir { ASC, DESC }

data class ImportSort(
    val field: ImportSortField = ImportSortField.NAME,
    val dir: ImportSortDir = ImportSortDir.ASC,
)

private fun defaultSplitPaths(): List<SplitPathItem> {
    return listOf()
}

@HiltViewModel
class ImportVM @Inject constructor(
    private val storageRepository: StorageRepository,
    private val importRepository: ImportRepository,
    private val permissionRepository: PermissionRepository,
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
    private val _selectedStorageId = MutableStateFlow(filterStorages(
        storageRepository.storages.value,
        importRepository.allowedStorageIds.value
    ).firstOrNull()?.id)
    private val _loadState = MutableStateFlow(CurrentStorageStateType.LOADING)
    private val _sort = MutableStateFlow(ImportSort())
    private val _searchQuery = MutableStateFlow("")
    private val _searchOpen = MutableStateFlow(false)

    /** Entries after the active search filter (empty query = everything). */
    private val filteredEntries = combine(_entries, _searchQuery) { entries, query ->
        if (query.isBlank()) {
            entries
        } else {
            entries.filter { entry -> entry.name.contains(query, ignoreCase = true) }
        }
    }.stateIn(viewModelScope, SharingStarted.Lazily, emptyList())
    private val _undoStack = MutableStateFlow(persistentListOf<String>())

    /**
     * True until the initial last-import-folder restore attempt settles
     * (a single-row backend read). While pending, [reload] is inert so
     * the collectors' immediate emissions cannot race the restore into
     * listing `/` first — the UI already shows the LOADING skeleton.
     */
    private var restorePending = true

    /**
     * True while the listing in flight is the *first open* of the
     * restored folder. Its failures fall back to the storage root
     * instead of rendering the error UI (see [handleLoadFailure]).
     */
    private var awaitingRememberedOpen = false

    /**
     * Storages the prepared import may actually use — the configured
     * storage list filtered by the source restriction
     * ([ImportRepository.allowedStorageIds]; `null` = unrestricted).
     * Drives default/fallback selection; disallowed storages stay
     * visible in the picker but render disabled.
     */
    private fun filterStorages(
        storages: List<Storage>,
        allowed: List<StorageId>?
    ): List<Storage> {
        if (allowed == null) {
            return storages
        }
        return storages.filter { storage -> allowed.contains(storage.id) }
    }

    private val selectableStorages = combine(
        storageRepository.storages,
        importRepository.allowedStorageIds
    ) {
            storages, allowed ->
        filterStorages(storages, allowed)
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Lazily,
        initialValue = filterStorages(
            storageRepository.storages.value,
            importRepository.allowedStorageIds.value
        )
    )

    /** All configured storages — the picker renders these, dimming the disallowed ones. */
    val storages = storageRepository.storages

    /** Storages excluded by the prepared import's source restriction (empty = unrestricted). */
    val disabledStorageIds = combine(
        storageRepository.storages,
        importRepository.allowedStorageIds
    ) {
            storages, allowed ->
        if (allowed == null) {
            emptySet()
        } else {
            storages
                .filter { storage -> !allowed.contains(storage.id) }
                .map { storage -> storage.id }
                .toSet()
        }
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Lazily,
        initialValue = emptySet()
    )

    val splitPaths = _splitPaths
    val selectedCount = _selected.combine(_entries) { selected, entries ->
        entries.count { entry -> selected.contains(entry.path) }
    }.stateIn(viewModelScope, SharingStarted.Lazily, 0)
    val entries = _entries.asStateFlow()

    /** The listing as shown: search-filtered, then sorted (dirs first). */
    val displayEntries = combine(filteredEntries, _sort) { list, sort ->
        list.sortedWith(sortComparator(sort))
    }.stateIn(viewModelScope, SharingStarted.Lazily, emptyList())
    val sort = _sort.asStateFlow()
    val searchQuery = _searchQuery.asStateFlow()
    val searchOpen = _searchOpen.asStateFlow()
    val selected = _selected.asStateFlow()
    val allowTypes = importRepository.allowTypes
    val selectedStorageId = _selectedStorageId.asStateFlow()
    val loadState = _loadState.asStateFlow()
    val canUndo =
        _undoStack.map {
            undoStack -> undoStack.isNotEmpty()
        }.stateIn(viewModelScope, SharingStarted.Lazily, false)


    init {
        viewModelScope.launch {
            restoreLastImportLoc()
            restorePending = false
            reload()
        }
        viewModelScope.launch {
            selectableStorages.collect { storages ->
                val storage = storages.find { storage -> storage.id == _selectedStorageId.value }
                if (storage == null) {
                    _selectedStorageId.value = storages.firstOrNull()?.id
                    // The previously selected storage is gone (removed,
                    // or newly outside this session's allowlist) — its
                    // deep path is meaningless under the replacement
                    // storage, so restart from the root.
                    _currentPath.value = "/"
                    _undoStack.value = persistentListOf()
                    _selected.update { selected -> selected.clear() }
                }

                reload()
            }
        }
        viewModelScope.launch {
            permissionRepository.havePermission.collect {
                reload()
            }
        }
    }

    /**
     * Seed the storage/path from the persisted last import folder when
     * it is still usable: the storage must exist and be allowed for this
     * import session (playlist allowlist). Anything else leaves the
     * defaults (first allowed storage, `/`). Read from
     * [com.kutedev.easemusicplayer.singleton.ImportRepository.allowedStorageIds] /
     * [storages] directly — the derived flows are `Lazily`-shared and
     * would still hold their initial values before the first subscriber.
     */
    private suspend fun restoreLastImportLoc() {
        val loc = try {
            importRepository.loadLastImportLoc()
        } catch (e: Throwable) {
            null
        } ?: return

        val allowed = importRepository.allowedStorageIds.value
        if (allowed != null && !allowed.contains(loc.storageId)) {
            return
        }
        if (storages.value.none { storage -> storage.id == loc.storageId }) {
            return
        }

        _selectedStorageId.value = loc.storageId
        _currentPath.value = loc.path
        _undoStack.value = persistentListOf()
        _selected.update { selected -> selected.clear() }
        awaitingRememberedOpen = true
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
        // Remember the folder this import came from — the next Import
        // entry reopens here. Fire-and-forget on the repository scope:
        // the page pops the route before calling this, so the VM (and
        // its viewModelScope) may already be on its way out.
        val storage = currentStorage()
        if (storage != null) {
            importRepository.saveLastImportLoc(
                StorageEntryLoc(storageId = storage.id, path = currentPath())
            )
        }
        importRepository.onFinish(v)
    }

    fun requestPermission() {
        permissionRepository.requestStoragePermission()
    }

    fun selectStorage(storageId: StorageId) {
        if (disabledStorageIds.value.contains(storageId)) {
            return
        }
        _selectedStorageId.value = storageId
        _undoStack.value = persistentListOf()

        navigateDirImpl("/")
    }

    fun toggleAll() {
        // Only selectable entries participate: dirs and entries whose type
        // the current import does not accept (e.g. a .wma during a music
        // import) have no checkbox and must neither be selected nor counted.
        // Scoped to the *visible* (search-filtered) set.
        val selectable = selectableEntries(filteredEntries.value, allowTypes.value).map { it.path }
        if (selectable.isEmpty()) {
            return
        }
        val allSelected = _selected.value.containsAll(selectable)
        _selected.update { selected ->
            if (allSelected) {
                selected.clear()
            } else {
                selected.clear().addAll(selectable)
            }
        }
    }

    fun setSort(field: ImportSortField, dir: ImportSortDir) {
        _sort.value = ImportSort(field, dir)
    }

    fun openSearch() {
        _searchOpen.value = true
    }

    fun setSearchQuery(query: String) {
        _searchQuery.value = query
    }

    /** Exit search mode; the query goes with it. */
    fun closeSearch() {
        _searchOpen.value = false
        _searchQuery.value = ""
    }

    /** Dirs always first; within each group, by the chosen key/direction.
     *  Timestamp-unknown entries always sort last (either direction). */
    private fun sortComparator(sort: ImportSort): Comparator<StorageEntry> {
        return compareByDescending<StorageEntry> { it.isDir }
            .thenComparator { a, b ->
                when (sort.field) {
                    ImportSortField.NAME -> {
                        val r = a.name.compareTo(b.name, ignoreCase = true)
                        if (sort.dir == ImportSortDir.DESC) -r else r
                    }
                    ImportSortField.CREATED -> compareTimestamp(a, b, { it.createdAt }, sort.dir)
                    ImportSortField.MODIFIED -> compareTimestamp(a, b, { it.modifiedAt }, sort.dir)
                }
            }
    }

    private fun compareTimestamp(
        a: StorageEntry,
        b: StorageEntry,
        keyOf: (StorageEntry) -> ULong?,
        dir: ImportSortDir,
    ): Int {
        val av = keyOf(a)
        val bv = keyOf(b)
        if (av == null && bv == null) return 0
        if (av == null) return 1
        if (bv == null) return -1
        return if (dir == ImportSortDir.DESC) bv.compareTo(av) else av.compareTo(bv)
    }

    private fun selectableEntries(
        entries: List<StorageEntry>,
        types: List<StorageEntryType>
    ): List<StorageEntry> {
        return entries.filter { entry -> !entry.isDir && types.contains(entry.entryTyp()) }
    }

    fun reload() {
        // Inert until the last-import-folder restore settles — the
        // collectors above fire immediately on collect, and their
        // reloads would race the restore into listing `/` first.
        if (restorePending) {
            return
        }
        val storage = currentStorage() ?: return

        // The local storage reads by raw path (list + get) — without
        // all-files access Android denies both for files contributed by
        // other apps, so gate on the permission first (the NEED_PERMISSION
        // state renders the grant prompt). Cloud storages browse as usual.
        if (storage.handle is StorageHandle.Local &&
            !permissionRepository.havePermission.value
        ) {
            _loadState.value = CurrentStorageStateType.NEED_PERMISSION
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
                    handleLoadFailure(CurrentStorageStateType.UNKNOWN_ERROR)
                }
                is ListStorageEntryChildrenResp.Ok -> {
                    awaitingRememberedOpen = false
                    _loadState.value = CurrentStorageStateType.OK
                    _entries.value = resp.data
                }

                ListStorageEntryChildrenResp.AuthenticationFailed -> {
                    handleLoadFailure(CurrentStorageStateType.AUTHENTICATION_FAILED)
                }

                ListStorageEntryChildrenResp.Timeout -> {
                    handleLoadFailure(CurrentStorageStateType.TIMEOUT)
                }

                ListStorageEntryChildrenResp.Unknown -> {
                    handleLoadFailure(CurrentStorageStateType.UNKNOWN_ERROR)
                }
            }
        }
    }

    /**
     * A failed listing is a hard error — except the *first* open of the
     * restored folder (deleted folder, reconfigured storage, transient
     * timeout): that silently falls back to the storage root. The saved
     * preference is deliberately kept so the next import entry retries
     * the folder; if the root also fails, the normal error UI shows.
     */
    private fun handleLoadFailure(state: CurrentStorageStateType) {
        if (awaitingRememberedOpen) {
            navigateDirImpl("/")
            return
        }
        _loadState.value = state
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
            // `_splitPaths` is Lazily-shared: before its first subscriber
            // (e.g. while the restore seeds a deep path under the LOADING
            // skeleton, before the breadcrumb composes) it still holds the
            // initial empty list — fall back to the raw path, which is
            // already normalized (it came from a StorageEntry or a
            // restored preference written by this same pipeline).
            return _currentPath.value.ifEmpty { "/" }
        }
        return p
    }

    private fun currentStorage(): Storage? {
        val storage = storages.value.find { storage -> storage.id == _selectedStorageId.value }
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
        // Past the initial open, failures are genuine errors (see
        // [handleLoadFailure]).
        awaitingRememberedOpen = false
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
