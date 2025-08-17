package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.repositories.EditStorageRepository
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
import uniffi.ease_client_backend.Storage
import uniffi.ease_client_backend.StorageConnectionTestResult
import uniffi.ease_client_backend.ctRemoveStorage
import uniffi.ease_client_backend.ctTestStorage
import uniffi.ease_client_backend.ctUpsertStorage
import uniffi.ease_client_schema.StorageType
import javax.inject.Inject



@HiltViewModel
class EditStorageVM @Inject constructor(
    private val bridge: Bridge,
    private val editStorageRepository: EditStorageRepository
) : ViewModel() {
    private val _removeModalOpen = MutableStateFlow(false)
    private val _testResult = MutableStateFlow(StorageConnectionTestResult.NONE)
    private var _testJob: Job? = null

    val form = editStorageRepository.form
    val musicCount = editStorageRepository.musicCount
    val title = editStorageRepository.title
    val validated = editStorageRepository.validated

    val removeModalOpen = _removeModalOpen.asStateFlow()
    val isCreated = form.map { form -> form.id == null }
        .stateIn(viewModelScope, SharingStarted.Lazily, true)
    val testResult = _testResult.asStateFlow()


    fun prepareFormCreate() {
        editStorageRepository.prepareFormCreate()
    }

    fun prepareFormEdit(storage: Storage) {
        editStorageRepository.prepareFormEdit(storage)
    }

    fun updateForm(block: (form: ArgUpsertStorage) -> ArgUpsertStorage) {
        editStorageRepository.updateForm(block)
    }

    fun changeType(typ: StorageType) {
        editStorageRepository.changeType(typ)
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
        viewModelScope.launch {
            editStorageRepository.remove()
        }
    }

    suspend fun finish(): Boolean {
        return editStorageRepository.finish()
    }

    private fun validate(): Boolean {
        return editStorageRepository.validate()
    }

    private fun resetTestResult() {
        _testJob?.cancel()
        _testJob = null
        _testResult.value = StorageConnectionTestResult.NONE
    }
}
