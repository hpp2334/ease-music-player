package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.StringResource


class ToastRepository constructor(
    private val scope: CoroutineScope
) {
    private val _toast = MutableSharedFlow<String>()
    private val _toastRes = MutableSharedFlow<StringResource>()

    val toast = _toast.asSharedFlow()
    val toastRes = _toastRes.asSharedFlow()

    fun emitToast(msg: String) {
        scope.launch {
            _toast.emit(msg)
        }
    }

    fun emitToastRes(res: StringResource) {
        scope.launch {
            _toastRes.emit(res)
        }
    }
}
