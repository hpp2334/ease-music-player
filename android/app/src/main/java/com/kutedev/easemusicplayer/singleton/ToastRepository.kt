package com.kutedev.easemusicplayer.singleton

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton


@Singleton
class ToastRepository @Inject constructor(
    private val scope: CoroutineScope
) {
    private val _toast = MutableSharedFlow<String>()
    private val _toastRes = MutableSharedFlow<Int>()

    val toast = _toast.asSharedFlow()
    val toastRes = _toastRes.asSharedFlow()

    fun emitToast(msg: String) {
        scope.launch {
            _toast.emit(msg)
        }
    }

    fun emitToastRes(resId: Int) {
        scope.launch {
            _toastRes.emit(resId)
        }
    }
}
