package com.kutedev.easemusicplayer.widgets.playlists

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.components.ImportCover
import com.kutedev.easemusicplayer.components.SimpleFormText
import com.kutedev.easemusicplayer.core.UIBridgeController
import com.kutedev.easemusicplayer.viewmodels.EaseViewModel
import uniffi.ease_client.PlaylistCreateWidget
import uniffi.ease_client.PlaylistEditWidget
import uniffi.ease_client_shared.CreatePlaylistMode

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
private fun FullImportHeader(
    text: String,
) {
    Text(
        text = text,
        fontSize = 10.sp,
    )
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FullImportBlock(
    evm: EaseViewModel
) {
    val bridge = UIBridgeController.current
    val state = evm.createPlaylistState.collectAsState().value

    if (!state.fullImported) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(6.dp))
                .clickable {
                    bridge.dispatchClick(PlaylistCreateWidget.Import)
                }
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .padding(0.dp, 32.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Icon(
                painter = painterResource(id = R.drawable.icon_download),
                contentDescription = null,
            )
            Box(
                modifier = Modifier.height(10.dp)
            )
            Text(
                text = stringResource(R.string.playlists_dialog_playlist_full_import_desc),
                fontSize = 12.sp,
                textAlign = TextAlign.Center
            )
        }
    } else {
        val musicCount = state.musicCount
        val musicCountSuffix = stringResource(R.string.music_count_unit)

        Column(
            modifier = Modifier
                .fillMaxWidth()
        ) {
            FullImportHeader(
                text = stringResource(R.string.playlists_dialog_import_info),
            )
            Text(
                text = "$musicCount $musicCountSuffix"
            )
            Box(modifier = Modifier.height(12.dp))
            FullImportHeader(
                text = stringResource(R.string.playlists_dialog_playlist_name),
            )
            SimpleFormText(
                label = null,
                value = state.name,
                onChange = { value ->
                    bridge.dispatchChangeText(PlaylistCreateWidget.Name, value)
                }
            )
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(0.dp),
                verticalArrangement = Arrangement.spacedBy(0.dp)
            ) {
                for (name in state.recommendPlaylistNames) {
                    EaseTextButton(
                        modifier = Modifier.widthIn(max = 120.dp),
                        text = name,
                        type = EaseTextButtonType.Default,
                        size = EaseTextButtonSize.Small,
                        disabled = false,
                        onClick = {
                            bridge.dispatchChangeText(PlaylistCreateWidget.Name, name)
                        },
                    )
                }
            }
            Box(modifier = Modifier.height(12.dp))
            FullImportHeader(
                text = stringResource(R.string.playlists_dialog_cover),
            )
            ImportCover(
                dataSourceKey = state.picture,
                onAdd = {
                    bridge.dispatchClick(PlaylistCreateWidget.Cover);
                },
                onRemove = {
                    bridge.dispatchClick(PlaylistCreateWidget.ClearCover);
                }
            )
        }
    }
}

@Composable
fun CreatePlaylistsDialog(
    evm: EaseViewModel
) {
    val bridge = UIBridgeController.current
    val state = evm.createPlaylistState.collectAsState().value
    val isOpen = state.modalOpen
    val mode = state.mode

    val onDismissRequest = {
        bridge.dispatchClick(PlaylistCreateWidget.CloseModal)
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
                        bridge.dispatchClick(PlaylistCreateWidget.Tab(CreatePlaylistMode.FULL));
                    }
                )
                Tab(
                    stringId = R.string.playlists_dialog_tab_empty,
                    isActive = mode == CreatePlaylistMode.EMPTY,
                    onClick = {
                        bridge.dispatchClick(PlaylistCreateWidget.Tab(CreatePlaylistMode.EMPTY));
                    }
                )
            }
            Box(modifier = Modifier.height(8.dp))
            if (state.mode == CreatePlaylistMode.FULL) {
                FullImportBlock(
                    evm = evm,
                )
            } else {
                SimpleFormText(
                    label = stringResource(R.string.playlists_dialog_playlist_name),
                    value = state.name,
                    onChange = { value ->
                        bridge.dispatchChangeText(PlaylistCreateWidget.Name, value)
                    }
                )
            }
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
                            onClick = {
                                bridge.dispatchClick(PlaylistCreateWidget.Reset);
                            }
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
                        disabled = !state.canSubmit,
                        onClick = {
                            bridge.dispatchClick(PlaylistCreateWidget.FinishCreate);
                            onDismissRequest()
                        }
                    )
                }
            }
        }
    }
}


@Composable
fun EditPlaylistsDialog(
    evm: EaseViewModel
) {
    val bridge = UIBridgeController.current
    val state = evm.editPlaylistState.collectAsState().value
    val isOpen = state.modalOpen

    val onDismissRequest = {
        bridge.dispatchClick(PlaylistEditWidget.CloseModal)
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

            FullImportHeader(
                text = stringResource(R.string.playlists_dialog_playlist_name),
            )
            SimpleFormText(
                label = null,
                value = state.name,
                onChange = { value ->
                    bridge.dispatchChangeText(PlaylistEditWidget.Name, value)
                }
            )
            Box(modifier = Modifier.height(12.dp))
            FullImportHeader(
                text = stringResource(R.string.playlists_dialog_cover),
            )
            ImportCover(
                dataSourceKey = state.picture,
                onAdd = {
                    bridge.dispatchClick(PlaylistEditWidget.Cover);
                },
                onRemove = {
                    bridge.dispatchClick(PlaylistEditWidget.ClearCover);
                }
            )
            Row(
                horizontalArrangement = Arrangement.End,
                modifier = Modifier
                    .fillMaxWidth()
            ) {
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
                        bridge.dispatchClick(PlaylistEditWidget.FinishEdit);
                    }
                )
            }
        }
    }
}