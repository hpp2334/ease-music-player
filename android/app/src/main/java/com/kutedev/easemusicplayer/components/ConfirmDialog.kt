package com.kutedev.easemusicplayer.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import com.kutedev.easemusicplayer.R

@Composable
fun ConfirmDialog(
    open: Boolean,
    onConfirm: () -> Unit,
    onCancel: () -> Unit,
    content: @Composable () -> Unit,
) {
    if (!open) {
        return
    }

    Dialog(onDismissRequest = onCancel) {
        Column(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surface)
                .padding(24.dp, 24.dp),
        ) {
            Text(
                text = stringResource(id = R.string.confirm_dialog_title),
                color = MaterialTheme.colorScheme.error,
            )
            Box(modifier = Modifier.height(4.dp))
            content()
            Row(
                horizontalArrangement = Arrangement.End,
                modifier = Modifier
                    .fillMaxWidth()
            ) {
                EaseTextButton(
                    text = stringResource(id = R.string.confirm_dialog_btn_cancel),
                    type = EaseTextButtonType.Primary,
                    size = EaseTextButtonSize.Medium,
                    onClick = onCancel
                )
                EaseTextButton(
                    text = stringResource(id = R.string.confirm_dialog_btn_ok),
                    type = EaseTextButtonType.Primary,
                    size = EaseTextButtonSize.Medium,
                    onClick = onConfirm
                )
            }
        }
    }
}