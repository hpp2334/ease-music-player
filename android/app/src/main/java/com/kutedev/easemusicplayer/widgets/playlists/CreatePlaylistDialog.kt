package com.kutedev.easemusicplayer.widgets.playlists

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.components.SimpleFormText
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.CreatePlaylistViewModel
import uniffi.ease_client.CreatePlaylistMode
import uniffi.ease_client.finishCreatePlaylist
import uniffi.ease_client.updateCreatePlaylistMode
import uniffi.ease_client.updateCreatePlaylistName

@Composable
private fun Tab(
    stringId: Int,
    isActive: Boolean,
    onClick: () -> Unit,
) {
    val activeColor = MaterialTheme.colorScheme.primary

    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .clickable { onClick() }
    ) {
        Text(
            modifier = Modifier
                .padding(8.dp, 0.dp),
            text = stringResource(id = stringId),
            fontSize = 11.sp,
            color = if (!isActive) {
                Color.Unspecified
            } else {
                activeColor
            }
        )
        if (isActive) {
            Box(
                modifier = Modifier
                    .width(16.dp)
                    .height(1.dp)
                    .offset(0.dp, (-4).dp)
                    .background(activeColor)
            )
        }
    }
}

@Composable
fun PlaylistsDialog(
    vm: CreatePlaylistViewModel
) {
    val state = vm.state.collectAsState().value
    val isOpen = vm.isOpen.collectAsState().value
    val mode = state.mode

    val onDismissRequest = {
        vm.closeDialog()
    }
    val onChangeTab = fun (mode: CreatePlaylistMode) {
        Bridge.invoke {
            updateCreatePlaylistMode(mode)
        }
    }

    if (!isOpen) {
        return
    }

    Dialog(
        onDismissRequest = onDismissRequest
    ) {
        Column(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surface)
                .padding(24.dp, 24.dp),
        ) {
            Row {
                Tab(
                    stringId = R.string.playlists_dialog_tab_full,
                    isActive = mode == CreatePlaylistMode.FULL,
                    onClick = {
                        onChangeTab(CreatePlaylistMode.FULL)
                    }
                )
                Tab(
                    stringId = R.string.playlists_dialog_tab_empty,
                    isActive = mode == CreatePlaylistMode.EMPTY,
                    onClick = {
                        onChangeTab(CreatePlaylistMode.EMPTY)
                    }
                )
            }
            Box(modifier = Modifier.height(8.dp))
            SimpleFormText(
                label = stringResource(R.string.playlists_dialog_playlist_name),
                value = state.name,
                onChange = {value -> Bridge.invoke { updateCreatePlaylistName(value) } }
            )
            Row(
                horizontalArrangement = Arrangement.SpaceBetween,
                modifier = Modifier
                    .fillMaxWidth()
            ) {
                Row {
                    if (state.fullImported && state.mode == CreatePlaylistMode.FULL) {
                        EaseTextButton(
                            text = stringResource(id = R.string.playlists_dialog_button_reset),
                            type = EaseTextButtonType.Primary,
                            size = EaseTextButtonSize.Medium,
                            onClick = {}
                        )
                    }
                }
                Row {
                    EaseTextButton(
                        text = stringResource(id = R.string.playlists_dialog_button_cancel),
                        type = EaseTextButtonType.Primary,
                        size = EaseTextButtonSize.Medium,
                        onClick = onDismissRequest
                    )
                    EaseTextButton(
                        text = stringResource(id = R.string.playlists_dialog_button_ok),
                        type = EaseTextButtonType.Primary,
                        size = EaseTextButtonSize.Medium,
                        onClick = {
                            Bridge.invoke {
                                finishCreatePlaylist()
                            }
                            onDismissRequest()
                        }
                    )
                }
            }
        }
    }
}