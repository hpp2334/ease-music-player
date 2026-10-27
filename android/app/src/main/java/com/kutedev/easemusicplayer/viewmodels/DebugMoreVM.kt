package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.BridgeMethods
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.ListLogFile
import com.kutedev.easemusicplayer.singleton.types.ListLogFiles
import javax.inject.Inject
import kotlin.collections.orEmpty


@HiltViewModel
class DebugMoreVM @Inject constructor(
    private val bridge: Bridge,
) : ViewModel() {
    private val _logs = MutableStateFlow<List<ListLogFile>>(emptyList())
    val logs = _logs.asStateFlow()

    fun reload() {
        viewModelScope.launch {
            val v = bridge.call(BridgeMethods.Debug.LIST_LOG_FILES).unwrapOrNull()?.payload
            _logs.value = v?.files.orEmpty()
        }
    }

    fun triggerRustError() {
        viewModelScope.launch {
            // triggerError / triggerPanic stay on callRaw — caller wants
            // the error envelope, not success.
            bridge.callRaw("debug.triggerError").unwrapOrNull()
        }
    }

    fun triggerRustAsyncError() {
        viewModelScope.launch {
            bridge.callRaw("debug.triggerError").unwrapOrNull()
        }
    }

    fun triggerRustPanic() {
        viewModelScope.launch {
            bridge.callRaw("debug.triggerPanic").unwrapOrNull()
        }
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
