package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import javax.inject.Inject

data class SleepModeState(
    val enabled: Boolean = false,
    val expiredMs: Long = 0
)


class SleepModeLeftTime(ms: Long) {
    private val remainingMs = if (ms > 0) ms else 0L
    private val totalMinutes = (remainingMs / (1000 * 60)).toInt()

    val hour = totalMinutes / 60
    val minute = totalMinutes % 60
}

@HiltViewModel
class SleepModeVM @Inject constructor() : ViewModel() {
    private val _state = MutableStateFlow(SleepModeState())
    private val _modalOpen = MutableStateFlow(false)
    private val _editLeftTime = MutableStateFlow(SleepModeLeftTime(0))

    val state = _state.asStateFlow()
    val modalOpen = _modalOpen.asStateFlow()
    val editLeftTime = _editLeftTime.asStateFlow()

    fun openModal(leftTime: SleepModeLeftTime) {
        _editLeftTime.value = leftTime
        _modalOpen.value = true
    }
    fun openModal() {
        openModal(SleepModeLeftTime(_state.value.expiredMs - System.currentTimeMillis()))
    }

    fun closeModal() {
        _modalOpen.value = false
    }

    fun remove() {
        _state.update { state ->
            state.copy(enabled = false, expiredMs = 0)
        }
    }

    fun set(hour: Int, minute: Int) {
        val newExpiredMs = hour.toLong() * 3600_000 + minute.toLong() * 60_000
        _state.update { state -> state.copy(enabled = true, expiredMs = newExpiredMs) }
    }
}
