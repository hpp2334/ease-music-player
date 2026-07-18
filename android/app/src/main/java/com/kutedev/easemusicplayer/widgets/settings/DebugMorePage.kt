package com.kutedev.easemusicplayer.widgets.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.viewmodels.DebugMoreVM

private val paddingX = SettingPaddingX


@Composable
private fun Item(
    title: String,
    onClick: () -> Unit
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() }
    ) {
        Box(modifier = Modifier.height(56.dp))
        Text(
            modifier = Modifier.padding(horizontal = paddingX),
            text = title,
            fontSize = 14.sp,
        )
    }
}

@Composable
fun DebugMorePage(
    debugMoreVM: DebugMoreVM = hiltViewModel()
) {
    Box(
        modifier = Modifier.fillMaxSize(),
    ) {
        Column {
            Text(
                modifier = Modifier.padding(start = paddingX, end = paddingX, top = 24.dp, bottom = 4.dp),
                text = stringResource(id = R.string.setting_debug),
                fontSize = 32.sp,
            )
            Box(modifier = Modifier.height(24.dp))
            Item(
                title = stringResource(id = R.string.debug_trigger_rs_err),
                onClick = {
                    debugMoreVM.triggerRustError()
                }
            )
            Item(
                title = stringResource(id = R.string.debug_trigger_rs_async_err),
                onClick = {
                    debugMoreVM.triggerRustAsyncError()
                }
            )
            Item(
                title = stringResource(id = R.string.debug_trigger_rs_panic),
                onClick = {
                    debugMoreVM.triggerRustPanic()
                }
            )
            Item(
                title = stringResource(id = R.string.debug_trigger_kt_exception),
                onClick = {
                    debugMoreVM.triggerKotlinError()
                }
            )
            Item(
                title = stringResource(id = R.string.debug_trigger_kt_async_exception),
                onClick = {
                    debugMoreVM.triggerKotlinAsyncError()
                }
            )
        }
    }
}