package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.ctsGetPreferencePlaymode
import uniffi.ease_client_backend.ctsSavePreferencePlaymode
import uniffi.ease_client_schema.PlayMode
import javax.inject.Inject
import javax.inject.Singleton


@Singleton
class PreferenceRepository @Inject constructor(
    private val bridge: Bridge,
    private val _scope: CoroutineScope
) {
    private val _playMode = MutableStateFlow(PlayMode.SINGLE)

    val playMode = _playMode.asStateFlow()

    fun savePlayMode(playMode: PlayMode) {
        bridge.runSync { ctsSavePreferencePlaymode(it, playMode) }

        reload()
    }

    fun reload() {
        bridge.runSync { ctsGetPreferencePlaymode(it) }?.let { _playMode.value = it }
    }
}