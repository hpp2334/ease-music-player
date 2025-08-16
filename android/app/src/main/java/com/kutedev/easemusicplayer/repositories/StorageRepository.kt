package com.kutedev.easemusicplayer.repositories

import com.kutedev.easemusicplayer.core.Bridge
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.ctGetRefreshToken
import javax.inject.Inject
import javax.inject.Singleton


@Singleton
class StorageRepository @Inject constructor(
    private val bridge: Bridge
) {
    private val _oauthRefreshToken = MutableStateFlow("")

    val oauthRefreshToken = _oauthRefreshToken.asStateFlow()

    suspend fun updateRefreshToken(code: String) {
        val token = ctGetRefreshToken(bridge.backend, code)
        _oauthRefreshToken.value = token
    }
}