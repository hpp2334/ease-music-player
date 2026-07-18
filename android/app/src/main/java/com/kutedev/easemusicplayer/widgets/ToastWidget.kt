package com.kutedev.easemusicplayer.widgets

import android.R
import android.widget.Toast
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.viewmodels.ToastVM
import kotlinx.coroutines.flow.collectLatest

@Composable
fun ToastFrame(
    toastVM: ToastVM = hiltViewModel()
) {

    val context = LocalContext.current
    val toast = remember {
        Toast.makeText(context, "", Toast.LENGTH_SHORT)
    }

    LaunchedEffect(Unit) {
        toastVM.toast.collect {
            msg ->
                toast.setText(msg)
                toast.cancel()
                toast.show()
        }
    }
    LaunchedEffect(Unit) {
        toastVM.toastRes.collect {
                resId ->
            toast.setText(context.getText(resId))
            toast.cancel()
            toast.show()
        }
    }
}