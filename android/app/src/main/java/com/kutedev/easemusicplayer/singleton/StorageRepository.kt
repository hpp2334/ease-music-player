package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.ArgUpsertStorage
import uniffi.ease_client_backend.Storage
import uniffi.ease_client_backend.ctGetRefreshToken
import uniffi.ease_client_backend.ctListStorage
import uniffi.ease_client_backend.ctUpsertStorage
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class StorageRepository @Inject constructor(
    private val bridge: Bridge,
    private val scope: CoroutineScope
) {
    private val _oauthRefreshToken = MutableStateFlow("")
    private val _storages = MutableStateFlow(listOf<Storage>())

    val oauthRefreshToken = _oauthRefreshToken.asStateFlow()
    val storages = _storages.asStateFlow()

    suspend fun updateRefreshToken(code: String) {
        val token = bridge.run { ctGetRefreshToken(it, code) } ?: return
        _oauthRefreshToken.value = token
    }

    suspend fun upsertStorage(arg: ArgUpsertStorage) {
        bridge.run { ctUpsertStorage(it, arg) }
        reload()
    }

    suspend fun reload() {
        _storages.value = bridge.run { ctListStorage(it) } ?: emptyList()
    }
}