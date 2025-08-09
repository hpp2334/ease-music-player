package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject

data class SleepModeState(
    val enabled: Boolean = false,
    val expiredMs: ULong = 0u
)

@HiltViewModel
class SleepModeVM @Inject constructor() : ViewModel() {
    private val _state = MutableStateFlow(SleepModeState())
    private val _modalOpen = MutableStateFlow(false)

    val state = _state.asStateFlow()
    val modalOpen = _modalOpen.asStateFlow()

    fun openModal() {}

    fun closeModal() {}
}
