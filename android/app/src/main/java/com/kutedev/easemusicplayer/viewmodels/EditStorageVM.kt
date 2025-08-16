package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.repositories.StorageRepository
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
import uniffi.ease_client_backend.ctUpsertStorage
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

fun defaultArgUpsertStorage(): ArgUpsertStorage {
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
    private val storageRepository: StorageRepository
) : ViewModel() {
    private val _title = MutableStateFlow("")
    private val _musicCount = MutableStateFlow(0L)
    private val _removeModalOpen = MutableStateFlow(false)
    private val _testResult = MutableStateFlow(StorageConnectionTestResult.NONE)
    private var _testJob: Job? = null

    private val _form = MutableStateFlow(defaultArgUpsertStorage())

    val title = _title.asStateFlow()
    val removeModalOpen = _removeModalOpen.asStateFlow()
    val isCreated = _form.map { form -> form.id == null }
        .stateIn(viewModelScope, SharingStarted.Lazily, true)
    val testResult = _testResult.asStateFlow()

    val form = _form.asStateFlow()
    val musicCount = _musicCount.asStateFlow()

    val validated = MutableStateFlow(Validated())

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
    }

    fun prepareFormCreate() {
        _form.value = defaultArgUpsertStorage()
        _title.value = ""
        _musicCount.value = 0
    }

    fun updateForm(block: (form: ArgUpsertStorage) -> ArgUpsertStorage) {
        _form.value = block(form.value.copy())
    }

    fun test() {
        resetTestResult()
        if (!validate()) {
            return
        }

        _testJob = viewModelScope.launch {
            _testResult.value = ctTestStorage(bridge.backend, form.value)

            delay(5000)
            resetTestResult()
        }
    }


    fun openRemoveModal() {
        _removeModalOpen.value = true
    }

    fun closeRemoveModal() {
        _removeModalOpen.value = false
    }

    fun remove() {
        val id = _form.value.id

        viewModelScope.launch {
            if (id != null) {
                ctRemoveStorage(bridge.backend, id)
            }
        }
    }

    suspend fun finish(): Boolean {
        if (!validate()) {
            return false
        }

        ctUpsertStorage(bridge.backend, _form.value)
        return true
    }

    private fun validate(): Boolean {
        val f = form.value
        validated.value = Validated(
            addrEmpty = f.addr.isBlank(),
            aliasEmpty = f.alias.isBlank(),
            usernameEmpty = !f.isAnonymous && f.username.isBlank(),
            passwordEmpty = !f.isAnonymous && f.password.isBlank(),
        )
        return validated.value.valid()
    }

    private fun resetTestResult() {
        _testJob?.cancel()
        _testJob = null
        _testResult.value = StorageConnectionTestResult.NONE
    }
}
