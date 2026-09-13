package com.kutedev.easemusicplayer.widgets.playlists

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.layout.size
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseCheckbox
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.components.ImportCover
import com.kutedev.easemusicplayer.components.SimpleFormText
import com.kutedev.easemusicplayer.viewmodels.CreatePlaylistVM
import com.kutedev.easemusicplayer.viewmodels.VImportStorageEntry
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RouteImport
import com.kutedev.easemusicplayer.singleton.RouteImportType
import com.kutedev.easemusicplayer.viewmodels.EditPlaylistVM
import com.kutedev.easemusicplayer.singleton.types.CreatePlaylistMode
import com.kutedev.easemusicplayer.singleton.types.DataSourceKey
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupMeta
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupId
import com.kutedev.easemusicplayer.singleton.types.Storage
import com.kutedev.easemusicplayer.singleton.types.StorageAllowlistMode
import com.kutedev.easemusicplayer.singleton.types.StorageId
import androidx.compose.runtime.LaunchedEffect

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

/** Exclusive-choice indicator ring (All / Specific storages). */
@Composable
private fun EaseRadioDot(
    selected: Boolean,
) {
    val borderColor = if (selected) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurface
    }

    Box(
        modifier = Modifier
            .size(16.dp)
            .border(1.dp, borderColor, RoundedCornerShape(999.dp)),
        contentAlignment = Alignment.Center
    ) {
        if (selected) {
            Box(
                modifier = Modifier
                    .size(8.dp)
                    .clip(RoundedCornerShape(999.dp))
                    .background(MaterialTheme.colorScheme.primary)
            )
        }
    }
}

@Composable
private fun AllowlistModeRow(
    text: String,
    selected: Boolean,
    onClick: () -> Unit,
) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(4.dp))
            .clickable { onClick() }
            .padding(0.dp, 6.dp)
    ) {
        EaseRadioDot(selected = selected)
        Text(
            text = text,
            fontSize = 12.sp,
        )
    }
}

@Composable
private fun AllowlistStorageRow(
    storage: Storage,
    checked: Boolean,
    locked: Boolean,
    onToggle: () -> Unit,
) {
    val item = VImportStorageEntry(storage)
    val dim = if (locked) {
        0.5F
    } else {
        1F
    }

    Row(
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(4.dp))
            .clickable(enabled = !locked) { onToggle() }
            .padding(2.dp, 6.dp)
            .alpha(dim)
    ) {
        Column(
            modifier = Modifier
                .weight(1.0F)
        ) {
            Text(
                text = item.name,
                fontSize = 12.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (item.subtitle.isNotBlank()) {
                Text(
                    text = item.subtitle,
                    fontSize = 10.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Box(modifier = Modifier.width(12.dp))
        EaseCheckbox(
            value = checked,
            onChange = { _ -> onToggle() },
            disabled = locked,
        )
    }
}

/**
 * Group picker for the create/edit playlist dialogs: exclusive choice
 * across all groups (radio rows). A playlist is always created in — and
 * can be moved between — groups.
 */
@Composable
private fun GroupPickerBlock(
    groups: List<PlaylistGroupMeta>,
    selected: PlaylistGroupId?,
    onSelect: (PlaylistGroupId) -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
    ) {
        FullImportHeader(
            text = stringResource(R.string.playlists_dialog_group),
        )
        for (group in groups) {
            AllowlistModeRow(
                text = group.title,
                selected = selected == group.id,
                onClick = { onSelect(group.id) },
            )
        }
    }
}

/**
 * The [Advanced] expandable section of the create/edit playlist dialogs:
 * the playlist's storage allowlist (import-source restriction). All is
 * the default; Specific multi-selects storages, with storages already
 * referenced by the playlist's musics locked on (they cannot be
 * unchecked).
 */
@Composable
private fun PlaylistAdvancedBlock(
    advancedOpen: Boolean,
    allowlistMode: StorageAllowlistMode,
    allowlistStorages: List<StorageId>,
    lockedStorages: List<StorageId>,
    storages: List<Storage>,
    onToggleAdvanced: () -> Unit,
    onUpdateMode: (StorageAllowlistMode) -> Unit,
    onToggleStorage: (StorageId) -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
    ) {
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(4.dp))
                .clickable { onToggleAdvanced() }
                .padding(0.dp, 8.dp)
        ) {
            Text(
                text = stringResource(R.string.playlists_dialog_advanced),
                fontSize = 10.sp,
            )
            Icon(
                painter = painterResource(
                    id = if (advancedOpen) {
                        R.drawable.icon_collapse
                    } else {
                        R.drawable.icon_forward
                    }
                ),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(16.dp)
            )
        }
        AnimatedVisibility(visible = advancedOpen) {
            Column {
                Box(modifier = Modifier.height(4.dp))
                FullImportHeader(
                    text = stringResource(R.string.playlists_dialog_allowlist_title),
                )
                Box(modifier = Modifier.height(4.dp))
                AllowlistModeRow(
                    text = stringResource(R.string.playlists_dialog_allowlist_all),
                    selected = allowlistMode == StorageAllowlistMode.ALL,
                    onClick = { onUpdateMode(StorageAllowlistMode.ALL) },
                )
                AllowlistModeRow(
                    text = stringResource(R.string.playlists_dialog_allowlist_specific),
                    selected = allowlistMode == StorageAllowlistMode.SPECIFIC,
                    onClick = { onUpdateMode(StorageAllowlistMode.SPECIFIC) },
                )
                AnimatedVisibility(visible = allowlistMode == StorageAllowlistMode.SPECIFIC) {
                    Column {
                        for (storage in storages) {
                            val locked = lockedStorages.contains(storage.id)
                            val checked = locked || allowlistStorages.contains(storage.id)
                            AllowlistStorageRow(
                                storage = storage,
                                checked = checked,
                                locked = locked,
                                onToggle = { onToggleStorage(storage.id) },
                            )
                        }
                        if ((allowlistStorages + lockedStorages).distinct().isEmpty()) {
                            Text(
                                text = stringResource(R.string.playlists_dialog_allowlist_empty),
                                fontSize = 10.sp,
                                color = MaterialTheme.colorScheme.error,
                            )
                        }
                    }
                }
            }
        }
    }
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
    val context = LocalContext.current
    val isOpen by createPlaylistVM.modalOpen.collectAsState()
    val mode by createPlaylistVM.mode.collectAsState()
    val name by createPlaylistVM.name.collectAsState()
    val fullImported by createPlaylistVM.fullImported.collectAsState()
    val canSubmit by createPlaylistVM.canSubmit.collectAsState()
    val advancedOpen by createPlaylistVM.advancedOpen.collectAsState()
    val allowlistMode by createPlaylistVM.allowlistMode.collectAsState()
    val allowlistStorages by createPlaylistVM.allowlistStorages.collectAsState()
    val lockedAllowlistStorages by createPlaylistVM.lockedAllowlistStorages.collectAsState()
    val storages by createPlaylistVM.storages.collectAsState()
    val groups by createPlaylistVM.groups.collectAsState()
    val groupId by createPlaylistVM.groupId.collectAsState()

    // The dialog may open before the first reload lands (groups still
    // empty) — default-select the first group once it arrives.
    LaunchedEffect(groups, groupId) {
        if (groupId == null && groups.isNotEmpty()) {
            createPlaylistVM.updateGroupId(groups.first().id)
        }
    }

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
            Box(modifier = Modifier.height(12.dp))
            GroupPickerBlock(
                groups = groups,
                selected = groupId,
                onSelect = { id -> createPlaylistVM.updateGroupId(id) },
            )
            Box(modifier = Modifier.height(12.dp))
            PlaylistAdvancedBlock(
                advancedOpen = advancedOpen,
                allowlistMode = allowlistMode,
                allowlistStorages = allowlistStorages,
                lockedStorages = lockedAllowlistStorages,
                storages = storages,
                onToggleAdvanced = {
                    createPlaylistVM.toggleAdvanced()
                },
                onUpdateMode = { value ->
                    createPlaylistVM.updateAllowlistMode(value)
                },
                onToggleStorage = { value ->
                    createPlaylistVM.toggleAllowlistStorage(value)
                },
            )
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
    val advancedOpen by editPlaylistVM.advancedOpen.collectAsState()
    val allowlistMode by editPlaylistVM.allowlistMode.collectAsState()
    val allowlistStorages by editPlaylistVM.allowlistStorages.collectAsState()
    val lockedAllowlistStorages by editPlaylistVM.lockedAllowlistStorages.collectAsState()
    val storages by editPlaylistVM.storages.collectAsState()
    val groups by editPlaylistVM.groups.collectAsState()
    val groupId by editPlaylistVM.groupId.collectAsState()

    // Defensive: a legacy orphan playlist (no group) or a dialog opened
    // before groups load — default-select the first group.
    LaunchedEffect(groups, groupId) {
        if (groupId == null && groups.isNotEmpty()) {
            editPlaylistVM.updateGroupId(groups.first().id)
        }
    }

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
            Box(modifier = Modifier.height(12.dp))
            GroupPickerBlock(
                groups = groups,
                selected = groupId,
                onSelect = { id -> editPlaylistVM.updateGroupId(id) },
            )
            Box(modifier = Modifier.height(12.dp))
            PlaylistAdvancedBlock(
                advancedOpen = advancedOpen,
                allowlistMode = allowlistMode,
                allowlistStorages = allowlistStorages,
                lockedStorages = lockedAllowlistStorages,
                storages = storages,
                onToggleAdvanced = {
                    editPlaylistVM.toggleAdvanced()
                },
                onUpdateMode = { value ->
                    editPlaylistVM.updateAllowlistMode(value)
                },
                onToggleStorage = { value ->
                    editPlaylistVM.toggleAllowlistStorage(value)
                },
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
