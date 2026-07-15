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
import java.util.Timer
import kotlin.concurrent.schedule
import org.jetbrains.compose.resources.StringResource
import org.jetbrains.compose.resources.stringResource

data class EaseContextMenuItem(
    val stringId: StringResource,
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
                        text = stringResource(item.stringId),
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