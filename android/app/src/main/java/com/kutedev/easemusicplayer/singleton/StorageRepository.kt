package com.kutedev.easemusicplayer.singleton

import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgUpsertStorage
import uniffi.ease_client_backend.Storage
import uniffi.ease_client_backend.ctGetRefreshToken
import uniffi.ease_client_backend.ctListStorage
import uniffi.ease_client_backend.ctRemoveStorage
import uniffi.ease_client_backend.ctUpsertStorage
import uniffi.ease_client_schema.StorageId
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class StorageRepository @Inject constructor(
    private val bridge: Bridge,
    private val scope: CoroutineScope
) {
    private val _oauthRefreshToken = MutableStateFlow("")
    private val _storages = MutableStateFlow(listOf<Storage>())
    private val _preRemoveStorageEvent = MutableSharedFlow<StorageId>()
    private val _onRemoveStorageEvent = MutableSharedFlow<Unit>()

    val oauthRefreshToken = _oauthRefreshToken.asStateFlow()
    val storages = _storages.asStateFlow()
    val preRemoveStorageEvent = _preRemoveStorageEvent.asSharedFlow()
    val onRemoveStorageEvent = _preRemoveStorageEvent.asSharedFlow()

    suspend fun updateRefreshToken(code: String) {
        val token = bridge.run { ctGetRefreshToken(it, code) } ?: return
        _oauthRefreshToken.value = token
    }

    suspend fun upsertStorage(arg: ArgUpsertStorage) {
        bridge.run { ctUpsertStorage(it, arg) }
        reload()
    }

    suspend fun remove(id: StorageId) {
        _preRemoveStorageEvent.emit(id)
        bridge.run { ctRemoveStorage(it, id) }
        _onRemoveStorageEvent.emit(Unit)
        reload()
    }

    suspend fun reload() {
        _storages.value = bridge.run { ctListStorage(it) } ?: emptyList()
    }
}