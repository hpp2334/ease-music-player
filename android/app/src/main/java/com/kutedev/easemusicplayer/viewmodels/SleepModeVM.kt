package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject

class SleepModeLeftTime(ms: Long) {
    private val remainingMs = if (ms > 0) ms else 0L
    private val totalMinutes = (remainingMs / (1000 * 60)).toInt()

    val hour = totalMinutes / 60
    val minute = totalMinutes % 60
}

@HiltViewModel
class SleepModeVM @Inject constructor(
    val playerRepository: PlayerRepository
) : ViewModel() {
    private val _modalOpen = MutableStateFlow(false)
    private val _editLeftTime = MutableStateFlow(SleepModeLeftTime(0))

    val state = playerRepository.sleepState

    val modalOpen = _modalOpen.asStateFlow()
    val editLeftTime = _editLeftTime.asStateFlow()

    fun openModal(leftTime: SleepModeLeftTime) {
        _editLeftTime.value = leftTime
        _modalOpen.value = true
    }

    fun openModal() {
        openModal(SleepModeLeftTime(state.value.expiredMs - System.currentTimeMillis()))
    }

    fun closeModal() {
        _modalOpen.value = false
    }

    fun remove() {
        playerRepository.cancelSleep()
    }

    fun set(hour: Int, minute: Int) {
        val newExpiredMs = System.currentTimeMillis() + hour.toLong() * 3600_000 + minute.toLong() * 60_000

        playerRepository.scheduleSleep(newExpiredMs)
    }
}
