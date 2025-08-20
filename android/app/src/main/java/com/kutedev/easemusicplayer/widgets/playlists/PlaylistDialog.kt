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
import androidx.compose.runtime.getValue
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
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.components.ImportCover
import com.kutedev.easemusicplayer.components.SimpleFormText
import com.kutedev.easemusicplayer.viewmodels.CreatePlaylistVM
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RouteImport
import com.kutedev.easemusicplayer.repositories.RouteImportType
import com.kutedev.easemusicplayer.viewmodels.EditPlaylistVM
import uniffi.ease_client_backend.CreatePlaylistMode
import uniffi.ease_client_schema.DataSourceKey

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
    createPlaylistVM: CreatePlaylistVM = hiltViewModel()
) {
    val navController = LocalNavController.current

    val musicCount by createPlaylistVM.musicCount.collectAsState()
    val name by createPlaylistVM.name.collectAsState()
    val recommendPlaylistNames by createPlaylistVM.recommendPlaylistNames.collectAsState()
    val cover by createPlaylistVM.cover.collectAsState()
    val fullImported by createPlaylistVM.fullImported.collectAsState()

    if (!fullImported) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(6.dp))
                .clickable {
                    createPlaylistVM.prepareImportCreate()
                    navController.navigate(RouteImport(RouteImportType.EditPlaylist))
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
                value = name,
                onChange = { value ->
                    createPlaylistVM.updateName(value)
                }
            )
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(0.dp),
                verticalArrangement = Arrangement.spacedBy(0.dp)
            ) {
                for (name in recommendPlaylistNames) {
                    EaseTextButton(
                        modifier = Modifier.widthIn(max = 120.dp),
                        text = name,
                        type = EaseTextButtonType.Default,
                        size = EaseTextButtonSize.Small,
                        disabled = false,
                        onClick = {
                            createPlaylistVM.updateName(name)
                        },
                    )
                }
            }
            Box(modifier = Modifier.height(12.dp))
            FullImportHeader(
                text = stringResource(R.string.playlists_dialog_cover),
            )
            ImportCover(
                dataSourceKey = cover.let { cover -> if (cover != null) DataSourceKey.AnyEntry(cover) else null },
                onAdd = {
                    navController.navigate(RouteImport(RouteImportType.EditPlaylistCover))
                },
                onRemove = {
                    createPlaylistVM.clearCover()
                }
            )
        }
    }
}

@Composable
fun CreatePlaylistsDialog(
    createPlaylistVM: CreatePlaylistVM = hiltViewModel()
) {
    val isOpen by createPlaylistVM.modalOpen.collectAsState()
    val mode by createPlaylistVM.mode.collectAsState()
    val name by createPlaylistVM.name.collectAsState()
    val fullImported by createPlaylistVM.fullImported.collectAsState()
    val canSubmit by createPlaylistVM.canSubmit.collectAsState()

    val onDismissRequest = {
        createPlaylistVM.closeModal()
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
                        createPlaylistVM.updateMode(CreatePlaylistMode.FULL)
                    }
                )
                Tab(
                    stringId = R.string.playlists_dialog_tab_empty,
                    isActive = mode == CreatePlaylistMode.EMPTY,
                    onClick = {
                        createPlaylistVM.updateMode(CreatePlaylistMode.EMPTY)
                    }
                )
            }
            Box(modifier = Modifier.height(8.dp))
            if (mode == CreatePlaylistMode.FULL) {
                FullImportBlock()
            } else {
                SimpleFormText(
                    label = stringResource(R.string.playlists_dialog_playlist_name),
                    value = name,
                    onChange = { value ->
                        createPlaylistVM.updateName(value)
                    }
                )
            }
            Row(
                horizontalArrangement = Arrangement.SpaceBetween,
                modifier = Modifier
                    .fillMaxWidth()
            ) {
                Row {
                    if (fullImported && mode == CreatePlaylistMode.FULL) {
                        EaseTextButton(
                            text = stringResource(id = R.string.playlists_dialog_button_reset),
                            type = EaseTextButtonType.Primary,
                            size = EaseTextButtonSize.Medium,
                            onClick = {
                                createPlaylistVM.reset()
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
                        disabled = !canSubmit,
                        onClick = {
                            createPlaylistVM.finish()
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
    editPlaylistVM: EditPlaylistVM = hiltViewModel()
) {
    val navController = LocalNavController.current

    val isOpen by editPlaylistVM.modalOpen.collectAsState()
    val name by editPlaylistVM.name.collectAsState()
    val cover by editPlaylistVM.cover.collectAsState()
    val canSubmit by editPlaylistVM.canSubmit.collectAsState()

    val onDismissRequest = {
        editPlaylistVM.closeModal()
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
                value = name,
                onChange = { value ->
                    editPlaylistVM.updateName(value)
                }
            )
            Box(modifier = Modifier.height(12.dp))
            FullImportHeader(
                text = stringResource(R.string.playlists_dialog_cover),
            )
            ImportCover(
                dataSourceKey = cover.let { cover -> if (cover != null) DataSourceKey.AnyEntry(cover) else null },
                onAdd = {
                    editPlaylistVM.prepareImportCover()
                    navController.navigate(RouteImport(RouteImportType.EditPlaylistCover))
                },
                onRemove = {
                    editPlaylistVM.clearCover()
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
                    disabled = !canSubmit,
                    onClick = {
                        editPlaylistVM.finish()
                    }
                )
            }
        }
    }
}