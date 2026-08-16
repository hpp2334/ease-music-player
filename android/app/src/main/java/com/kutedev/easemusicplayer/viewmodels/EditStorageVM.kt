package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.BridgeMethods
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.singleton.StorageProvider
import com.kutedev.easemusicplayer.singleton.StorageRepository
import com.kutedev.easemusicplayer.singleton.ToastRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.ArgUpsertWebdavStorage
import com.kutedev.easemusicplayer.singleton.types.StorageConnectionTestResult
import com.kutedev.easemusicplayer.singleton.types.StorageHandle
import com.kutedev.easemusicplayer.singleton.types.StorageId
import javax.inject.Inject


data class Validated(
    val addrEmpty: Boolean = false,
    val aliasEmpty: Boolean = false,
    val usernameEmpty: Boolean = false,
    val passwordEmpty: Boolean = false,
) {
    fun valid(): Boolean {
        return !addrEmpty && !aliasEmpty && !usernameEmpty && !passwordEmpty
    }
}

private fun defaultArgUpsertWebdavStorage(): ArgUpsertWebdavStorage {
    return ArgUpsertWebdavStorage(
        id = null,
        addr = "",
        alias = "",
        username = "",
        password = "",
        isAnonymous = true,
    )
}

/** Resolved view descriptor for an edited plugin storage (edit mode only). */
data class EditPluginView(
    val viewAssetPath: String,
    val pluginId: String,
    val pluginStorageId: String,
)

private data class EditPluginHandle(
    val pluginId: String,
    val pluginStorageId: String,
)


@HiltViewModel
class EditStorageVM @Inject constructor(
    private val bridge: Bridge,
    private val storageRepository: StorageRepository,
    private val pluginRepository: PluginRepository,
    private val toastRepository: ToastRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val _title = MutableStateFlow("")
    private val _musicCount = MutableStateFlow(0uL)
    private val _form = MutableStateFlow(defaultArgUpsertWebdavStorage())

    private val _validated = MutableStateFlow(Validated())
    private val _removeModalOpen = MutableStateFlow(false)
    private val _testResult = MutableStateFlow(StorageConnectionTestResult.NONE)
    private var _testJob: Job? = null
    // True when editing an existing JS-plugin storage. In create mode the
    // type is chosen via the UI chooser, so this only reflects the loaded
    // storage's kind.
    private val _pluginMode = MutableStateFlow(false)
    // For an edited plugin storage: its (pluginId, pluginStorageId). Drives
    // the derived `editPluginView` (resolved against `storageProviders`).
    private val _editPluginHandle = MutableStateFlow<EditPluginHandle?>(null)
    private val _removedEvent = MutableSharedFlow<Unit>()

    val form = _form.asStateFlow()
    val musicCount = _musicCount.asStateFlow()
    val title = _title.asStateFlow()
    val validated = _validated.asStateFlow()
    val pluginMode = _pluginMode.asStateFlow()
    /** Discovered plugin storage providers (drives the create-mode chooser). */
    val storageProviders = pluginRepository.storageProviders
    /**
     * For an edited plugin storage: the resolved view JS asset path + ids
     * (null in create mode, or until `scanPlugins` resolves the provider).
     * `EditStoragesPage` renders this into a `TurView` stamped with the
     * storage id so `ease.context.storageId$` is non-null (edit mode).
     */
    val editPluginView: kotlinx.coroutines.flow.StateFlow<EditPluginView?> =
        combine(_editPluginHandle, pluginRepository.storageProviders) { handle, providers ->
            if (handle == null) {
                null
            } else {
                providers
                    .firstOrNull { it.pluginId == handle.pluginId }
                    ?.viewAssetPath
                    ?.let { path -> EditPluginView(path, handle.pluginId, handle.pluginStorageId) }
            }
        }.stateIn(viewModelScope, SharingStarted.Eagerly, null)
    /** Fires when the edited storage disappears from the list (the plugin
     *  backend called `ease.context.removeStorage`, or the trash removed it).
     *  `EditStoragesPage` collects this to pop back. */
    val removedEvent = _removedEvent.asSharedFlow()

    val removeModalOpen = _removeModalOpen.asStateFlow()
    /** Fires when a JS plugin OAuth exchange mints a new storage row.
     *  `EditStoragesPage` collects this to pop back from the setup form. */
    val pluginConnectedEvent = storageRepository.pluginConnectedEvent
    val isCreated = form.map { form -> form.id == null }
        .stateIn(viewModelScope, SharingStarted.Lazily, true)
    val testResult = _testResult.asStateFlow()

    init {
        _form.value = defaultArgUpsertWebdavStorage()
        _title.value = ""
        _musicCount.value = 0u

        val id: Long? = savedStateHandle["id"]
        // The route determines mode: `RouteCreateStorage` passes no id
        // (id == null -> create new), `RouteEditStorage/{id}` passes a real
        // storage id (edit existing). No sentinel, no ambiguity.
        val storage = if (id == null) {
            null
        } else {
            storageRepository.storages.value.find { v -> v.id == StorageId(id) }
        }
        if (storage != null) {
            val isPlugin = storage.handle is StorageHandle.Plugin
            _pluginMode.value = isPlugin
            if (isPlugin) {
                val handle = storage.handle as StorageHandle.Plugin
                _editPluginHandle.value = EditPluginHandle(
                    pluginId = handle.pluginId.id,
                    pluginStorageId = handle.pluginStorageId.id,
                )
            }
            // Password is write-only: blank on edit means "keep the existing
            // secret" (the backend rotates only when a non-empty value is sent).
            _form.value = ArgUpsertWebdavStorage(
                id = storage.id,
                addr = storage.addr ?: "",
                alias = storage.alias,
                username = storage.username ?: "",
                password = "",
                isAnonymous = storage.isAnonymous ?: false,
            )
            _title.value = VImportStorageEntry(storage).name
            _musicCount.value = storage.musicCount

            // Pop the edit page when the storage is removed out from under
            // us (view-side disconnect via the plugin backend, or the
            // top-bar trash). `wasPresent` guards the initial load; `done`
            // stops repeats after the first removal (collect is
            // crossinline so a non-local return isn't allowed).
            viewModelScope.launch {
                var wasPresent = false
                var done = false
                storageRepository.storages.collect { list ->
                    if (!done) {
                        val editId = _form.value.id
                        if (editId != null) {
                            val present = list.any { it.id == editId }
                            if (wasPresent && !present) {
                                _removedEvent.tryEmit(Unit)
                                done = true
                            }
                            wasPresent = present
                        }
                    }
                }
            }
        }

        // Discover plugin storage providers for the create-mode chooser.
        viewModelScope.launch {
            pluginRepository.scanPlugins()
        }
    }

    fun test() {
        resetTestResult()
        if (!validate()) {
            return
        }
        _testResult.value = StorageConnectionTestResult.TESTING

        _testJob = viewModelScope.launch {
            val result: StorageConnectionTestResult = try {
                bridge.call(BridgeMethods.StorageWebdav.TEST, form.value).unwrapOrThrow().payload
            } catch (e: Throwable) {
                StorageConnectionTestResult.OTHER_ERROR
            }
            _testResult.value = result
            sendTestToast()

            delay(5000)
            resetTestResult()
        }
    }

    private fun sendTestToast() {
        val testing = _testResult.value
        if (testing == StorageConnectionTestResult.NONE || testing == StorageConnectionTestResult.TESTING) {
            return;
        }

        when (testing) {
            StorageConnectionTestResult.SUCCESS -> {
                toastRepository.emitToastRes(R.string.storage_edit_testing_toast_success)
            }
            StorageConnectionTestResult.TIMEOUT -> {
                toastRepository.emitToastRes(R.string.storage_edit_testing_toast_timeout)
            }
            StorageConnectionTestResult.UNAUTHORIZED -> {
                toastRepository.emitToastRes(R.string.storage_edit_testing_toast_unauth)
            }
            StorageConnectionTestResult.OTHER_ERROR -> {
                toastRepository.emitToastRes(R.string.storage_edit_testing_toast_other_error)
            }
            else -> {}
        }
    }


    fun openRemoveModal() {
        _removeModalOpen.value = true
    }

    fun closeRemoveModal() {
        _removeModalOpen.value = false
    }

    fun updateForm(block: (form: ArgUpsertWebdavStorage) -> ArgUpsertWebdavStorage) {
        _form.value = block(form.value.copy())
    }

    private fun validate(): Boolean {
        val f = form.value
        _validated.value = Validated(
            addrEmpty = f.addr.isBlank(),
            aliasEmpty = f.alias.isBlank(),
            usernameEmpty = !f.isAnonymous && f.username.isBlank(),
            // Password required on create for non-anonymous; blank on edit = keep.
            passwordEmpty = !f.isAnonymous && f.id == null && f.password.isBlank(),
        )
        return _validated.value.valid()
    }

    fun remove() {
        val id = _form.value.id

        if (id != null) {
            viewModelScope.launch {
                if (_pluginMode.value) {
                    storageRepository.pluginRemoveInstance(id)
                } else {
                    storageRepository.remove(id)
                }
            }
        }
    }

    suspend fun finish(): Boolean {
        if (!validate()) {
            return false
        }

        val form = _form.value
        val ok = if (form.id == null) {
            storageRepository.createStorage(form)
        } else {
            storageRepository.updateStorage(form)
        }
        if (!ok) {
            toastRepository.emitToastRes(R.string.storage_edit_save_failed)
            return false
        }
        return true
    }

    private fun resetTestResult() {
        _testJob?.cancel()
        _testJob = null
        _testResult.value = StorageConnectionTestResult.NONE
    }
}
