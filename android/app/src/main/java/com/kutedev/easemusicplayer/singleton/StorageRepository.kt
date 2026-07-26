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
    val onRemoveStorageEvent = _onRemoveStorageEvent.asSharedFlow()

    suspend fun updateRefreshToken(code: String) {
        val token = bridge.run { ctGetRefreshToken(it, code) } ?: return
        _oauthRefreshToken.value = token
    }

    /**
     * Create a new storage. `arg.id` must be null — the database assigns the id.
     * Returns true on success, false if the backend call failed (the bridge
     * swallows the exception; callers expecting the error message should use
     * `bridge.runRaw` directly).
     */
    suspend fun createStorage(arg: ArgUpsertStorage): Boolean {
        require(arg.id == null) { "createStorage: arg.id must be null" }
        bridge.run { ctUpsertStorage(it, arg) } ?: return false
        reload()
        return true
    }

    /**
     * Update an existing storage. `arg.id` must be non-null and refer to a
     * real persisted storage row (i.e. not the synthetic Local sentinel —
     * the service layer rejects that with an explicit error).
     */
    suspend fun updateStorage(arg: ArgUpsertStorage): Boolean {
        require(arg.id != null) { "updateStorage: arg.id must be non-null" }
        bridge.run { ctUpsertStorage(it, arg) } ?: return false
        reload()
        return true
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