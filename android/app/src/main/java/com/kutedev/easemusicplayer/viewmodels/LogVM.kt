package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.BridgeMethods
import com.kutedev.easemusicplayer.singleton.ToastRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.ListLogFile
import com.kutedev.easemusicplayer.singleton.types.ListLogFiles
import javax.inject.Inject


@HiltViewModel
class LogVM @Inject constructor(
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
}
