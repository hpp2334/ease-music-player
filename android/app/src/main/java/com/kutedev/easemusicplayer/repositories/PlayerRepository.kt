package com.kutedev.easemusicplayer.repositories

import android.content.Context
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.easeLog
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.max


data class SleepModeState(
    val enabled: Boolean = false,
    val expiredMs: Long = 0
)

@Singleton
class PlayerRepository @Inject constructor(
    private val _scope: CoroutineScope
) {
    private val _sleep = MutableStateFlow(SleepModeState())

    private var _job: Job? = null

    val sleepState = _sleep.asStateFlow()


    fun scheduleSleep(newExpiredMs: Long) {
        _job?.cancel()

        val delayMs = max(newExpiredMs - System.currentTimeMillis(), 0)
        _job = _scope.launch {
            _sleep.update { state -> state.copy(enabled = true, expiredMs = newExpiredMs) }
            easeLog("schedule sleep")
            kotlinx.coroutines.delay(delayMs)
            easeLog("sleep scheduled")
            _sleep.update { state -> state.copy(enabled = false, expiredMs = 0) }
        }
    }

    fun cancelSleep() {
        _job?.cancel()
        _job = null
        _sleep.update { state -> state.copy(enabled = false, expiredMs = 0) }
    }
}
