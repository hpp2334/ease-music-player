package com.kutedev.easemusicplayer.components

import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import com.kutedev.easemusicplayer.R
import java.util.Timer
import kotlin.concurrent.schedule

data class EaseContextMenuItem(
    val stringId: Int,
    val onClick: () -> Unit,
    val isError: Boolean = false
) {
}

@Composable
fun EaseContextMenu(
    expanded: Boolean,
    onDismissRequest: () -> Unit,
    items: List<EaseContextMenuItem>
) {
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismissRequest
    ) {
        for (item in items) {
            DropdownMenuItem(
                text = {
                    Text(
                        text = stringResource(id = item.stringId),
                        color = if (!item.isError) { Color.Unspecified } else { MaterialTheme.colorScheme.error }
                    )
                },
                onClick = {
                    Timer("Close ContextMenu", false).schedule(160) {
                        onDismissRequest()
                    }
                    item.onClick()
                }
            )
        }
    }
}