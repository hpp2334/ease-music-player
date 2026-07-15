package com.kutedev.easemusicplayer.widgets

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import com.kutedev.easemusicplayer.platform.platformShowToast
import com.kutedev.easemusicplayer.viewmodels.ToastVM
import org.jetbrains.compose.resources.getString
import org.koin.compose.viewmodel.koinViewModel

@Composable
fun ToastFrame(
    toastVM: ToastVM = koinViewModel()
) {
    LaunchedEffect(Unit) {
        toastVM.toast.collect { msg ->
            platformShowToast(msg)
        }
    }
    LaunchedEffect(Unit) {
        toastVM.toastRes.collect { res ->
            val msg = getString(res)
            platformShowToast(msg)
        }
    }
}
