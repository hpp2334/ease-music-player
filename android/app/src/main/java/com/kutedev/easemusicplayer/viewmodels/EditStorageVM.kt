package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.Storage
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

@HiltViewModel
class EditStorageVM @Inject constructor() : ViewModel() {
    private val _title = MutableStateFlow("")
    private val _modalOpen = MutableStateFlow(false)
    private val _musicCount = MutableStateFlow(0)
    private val _isCreated = MutableStateFlow(false)

    val title = _title.asStateFlow()
    val modalOpen = _modalOpen.asStateFlow()
    val musicCount = _musicCount.asStateFlow()
    val isCreated = _isCreated.asStateFlow()

    val form = MutableStateFlow(Storage(
        id = StorageId(0),
        addr = "",
        alias = "",
        username = "",
        password = "",
        isAnonymous = true,
        typ = StorageType.WEBDAV,
        musicCount = 0uL,
    ))

    val validated = MutableStateFlow(Validated())


    fun openModal() {}

    fun closeModal() {}

    fun remove() {}

    fun selectStorage(typ: StorageType) {}

    fun finish() {}
}