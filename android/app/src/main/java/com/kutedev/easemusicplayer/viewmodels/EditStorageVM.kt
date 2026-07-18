package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import com.kutedev.easemusicplayer.singleton.ToastRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgUpsertStorage
import uniffi.ease_client_backend.StorageConnectionTestResult
import uniffi.ease_client_backend.ctRemoveStorage
import uniffi.ease_client_backend.ctTestStorage
import uniffi.ease_client_schema.StorageId
import uniffi.ease_client_schema.StorageType
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

private fun defaultArgUpsertStorage(): ArgUpsertStorage {
    return ArgUpsertStorage(
        id = null,
        addr = "",
        alias = "",
        username = "",
        password = "",
        isAnonymous = true,
        typ = StorageType.WEBDAV,
    )
}


@HiltViewModel
class EditStorageVM @Inject constructor(
    private val bridge: Bridge,
    private val storageRepository: StorageRepository,
    private val toastRepository: ToastRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val _title = MutableStateFlow("")
    private val _musicCount = MutableStateFlow(0uL)
    private val _form = MutableStateFlow(defaultArgUpsertStorage())
    private var _formBackups = HashMap<StorageType, ArgUpsertStorage>()

    private val _validated = MutableStateFlow(Validated())
    private val _removeModalOpen = MutableStateFlow(false)
    private val _testResult = MutableStateFlow(StorageConnectionTestResult.NONE)
    private var _testJob: Job? = null

    val form = _form.asStateFlow()
    val musicCount = _musicCount.asStateFlow()
    val title = _title.asStateFlow()
    val validated = _validated.asStateFlow()

    val removeModalOpen = _removeModalOpen.asStateFlow()
    val isCreated = form.map { form -> form.id == null }
        .stateIn(viewModelScope, SharingStarted.Lazily, true)
    val testResult = _testResult.asStateFlow()

    init {
        viewModelScope.launch {
            storageRepository.oauthRefreshToken.collect {
                    refreshToken ->
                updateForm { storage ->
                    if (storage.typ == StorageType.ONE_DRIVE) {
                        storage.password = refreshToken
                    }
                    storage
                }
            }
        }

        _form.value = defaultArgUpsertStorage()
        _title.value = ""
        _musicCount.value = 0u

        val id: Long? = savedStateHandle["id"]
        val storage = storageRepository.storages.value.find { v -> id != null && v.id == StorageId(id) }
        if (storage != null) {
            _form.value = ArgUpsertStorage(
                id = storage.id,
                addr = storage.addr,
                alias = storage.alias,
                username = storage.username,
                password = storage.password,
                isAnonymous = storage.isAnonymous,
                typ = storage.typ
            )
            _title.value = VImportStorageEntry(storage).name
            _musicCount.value = storage.musicCount
        }
    }

    fun test() {
        resetTestResult()
        if (!validate()) {
            return
        }
        _testResult.value = StorageConnectionTestResult.TESTING

        _testJob = viewModelScope.launch {
            _testResult.value = bridge.runRaw { ctTestStorage(it, form.value) }
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

    fun updateForm(block: (form: ArgUpsertStorage) -> ArgUpsertStorage) {
        _form.value = block(form.value.copy())
    }

    fun changeType(typ: StorageType) {
        _formBackups.set(_form.value.typ, _form.value.copy())

        val backup = _formBackups.get(typ)
        if (backup != null) {
            _form.value = backup
        } else {
            val newForm = ArgUpsertStorage(
                id = _form.value.id,
                addr = "",
                alias = _form.value.alias,
                username = "",
                password = "",
                isAnonymous = false,
                typ = typ
            )
            _form.value = newForm
        }
        _validated.value = Validated()
    }

    private fun validate(): Boolean {
        val f = form.value
        _validated.value = Validated(
            addrEmpty = if (f.typ == StorageType.WEBDAV) {  f.addr.isBlank() } else { false },
            aliasEmpty = if (f.typ == StorageType.WEBDAV) { false } else { f.alias.isBlank() },
            usernameEmpty = if (f.typ == StorageType.WEBDAV) { !f.isAnonymous && f.username.isBlank() } else { false },
            passwordEmpty = if (f.typ == StorageType.WEBDAV) { !f.isAnonymous && f.password.isBlank() } else { f.password.isBlank() },
        )
        return _validated.value.valid()
    }

    fun remove() {
        val id = _form.value.id

        if (id != null) {
            viewModelScope.launch {
                storageRepository.remove(id)
            }
        }
    }

    suspend fun finish(): Boolean {
        if (!validate()) {
            return false
        }

        storageRepository.upsertStorage(_form.value)
        return true
    }

    private fun resetTestResult() {
        _testJob?.cancel()
        _testJob = null
        _testResult.value = StorageConnectionTestResult.NONE
    }
}
