package com.kutedev.easemusicplayer.repositories

import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.core.Bridge
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgUpsertStorage
import uniffi.ease_client_backend.Storage
import uniffi.ease_client_backend.ctGetRefreshToken
import uniffi.ease_client_backend.ctListStorage
import uniffi.ease_client_backend.ctUpsertStorage
import uniffi.ease_client_schema.StorageType
import javax.inject.Inject
import javax.inject.Singleton
@Singleton
class StorageRepository @Inject constructor(
    private val bridge: Bridge,
) {
    private val _oauthRefreshToken = MutableStateFlow("")
    private val _storages = MutableStateFlow(listOf<Storage>())

    val oauthRefreshToken = _oauthRefreshToken.asStateFlow()
    val storages = _storages.asStateFlow()

    suspend fun updateRefreshToken(code: String) {
        val token = ctGetRefreshToken(bridge.backend, code)
        _oauthRefreshToken.value = token
    }

    suspend fun upsertStorage(arg: ArgUpsertStorage) {
        ctUpsertStorage(bridge.backend, arg)
        reload()
    }

    suspend fun reload() {
        _storages.value = ctListStorage(bridge.backend)
    }
}