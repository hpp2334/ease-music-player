package com.kutedev.easemusicplayer.repositories

import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.VImportStorageEntry
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgUpsertStorage
import uniffi.ease_client_backend.Storage
import uniffi.ease_client_backend.ctRemoveStorage
import uniffi.ease_client_backend.ctUpsertStorage
import uniffi.ease_client_schema.StorageType
import javax.inject.Inject
import javax.inject.Singleton


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

@Singleton
class EditStorageRepository @Inject constructor(
    private val bridge: Bridge,
    private val storageRepository: StorageRepository,
    private val _scope: CoroutineScope
) {

    private val _title = MutableStateFlow("")
    private val _musicCount = MutableStateFlow(0uL)
    private val _form = MutableStateFlow(defaultArgUpsertStorage())
    private var _formBackups = HashMap<StorageType, ArgUpsertStorage>()

    private val _validated = MutableStateFlow(Validated())

    val title = _title.asStateFlow()
    val form = _form.asStateFlow()
    val musicCount = _musicCount.asStateFlow()
    val validated = _validated.asStateFlow()

    init {
        _scope.launch {
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
        _musicCount.value = 0u
    }

    fun prepareFormEdit(storage: Storage) {
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

    fun validate(): Boolean {
        val f = form.value
        _validated.value = Validated(
            addrEmpty = f.addr.isBlank(),
            aliasEmpty = f.alias.isBlank(),
            usernameEmpty = !f.isAnonymous && f.username.isBlank(),
            passwordEmpty = !f.isAnonymous && f.password.isBlank(),
        )
        return _validated.value.valid()
    }

    suspend fun remove() {
        val id = _form.value.id

        if (id != null) {
            ctRemoveStorage(bridge.backend, id)
        }
    }

    suspend fun finish(): Boolean {
        if (!validate()) {
            return false
        }

        ctUpsertStorage(bridge.backend, _form.value)
        return true
    }
}