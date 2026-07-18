package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.Bridge
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ListLogFile
import uniffi.ease_client_backend.ctTriggerError
import uniffi.ease_client_backend.ctsListLogFiles
import uniffi.ease_client_backend.ctsTriggerError
import uniffi.ease_client_backend.ctsTriggerPanic
import javax.inject.Inject
import kotlin.collections.orEmpty


@HiltViewModel
class DebugMoreVM @Inject constructor(
    private val bridge: Bridge
) : ViewModel() {
    private val _logs: MutableStateFlow<List<ListLogFile>> = MutableStateFlow(emptyList())

    val logs = _logs.asStateFlow()

    fun reload() {
        val v = bridge.runSync { ctsListLogFiles(it) }
        _logs.value = v?.files.orEmpty()
    }

    fun triggerRustError() {
        bridge.runSyncRaw { ctsTriggerError(it) }
    }

    fun triggerRustAsyncError() {
        viewModelScope.launch {
            bridge.runRaw { ctTriggerError(it) }
        }
    }
    fun triggerRustPanic() {
        bridge.runSyncRaw { ctsTriggerPanic(it) }
    }

    fun triggerKotlinError() {
        throw RuntimeException("Kotlin error triggered")
    }

    fun triggerKotlinAsyncError() {
        viewModelScope.launch {
            throw RuntimeException("Kotlin async error triggered")
        }
    }
}
