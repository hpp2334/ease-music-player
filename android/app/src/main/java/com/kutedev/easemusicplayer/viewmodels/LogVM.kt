package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.ToastRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.ListLogFile
import uniffi.ease_client_backend.ctsListLogFiles
import javax.inject.Inject


@HiltViewModel
class LogVM @Inject constructor(
    private val bridge: Bridge
) : ViewModel() {
    private val _logs: MutableStateFlow<List<ListLogFile>> = MutableStateFlow(emptyList())

    val logs = _logs.asStateFlow()

    fun reload() {
        val v = bridge.runSync { ctsListLogFiles(it) }
        _logs.value = v?.files.orEmpty()
    }
}
