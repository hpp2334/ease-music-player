package com.kutedev.easemusicplayer.widgets.musics

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseCheckbox
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.CurrentStorageEntriesViewModel
import uniffi.ease_client.StorageImportAction
import uniffi.ease_client.StorageImportWidget
import uniffi.ease_client.StorageUpsertWidget
import uniffi.ease_client.VCurrentStorageEntriesStateStorageItem
import uniffi.ease_client.VCurrentStorageEntry
import uniffi.ease_client.VSplitPathItem
import uniffi.ease_client.ViewAction
import uniffi.ease_client_shared.CurrentStorageStateType
import uniffi.ease_client_shared.StorageEntryType

@Composable
private fun ImportEntriesSkeleton() {
    @Composable
    fun Block(
        width: Dp,
        height: Dp,
    ) {
        val color = MaterialTheme.colorScheme.surfaceVariant
        Box(modifier = Modifier
            .width(width)
            .height(height)
            .clip(RoundedCornerShape(6.dp))
            .background(color)
        )
    }

    @Composable
    fun FolderItem() {
        Row(
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.height(30.dp)
        ) {
            Block(width = 30.dp, height = 30.dp)
            Column(
                verticalArrangement = Arrangement.SpaceBetween,
                modifier = Modifier.fillMaxHeight()
            ) {
                Block(width = 138.dp, height = 17.dp)
                Block(width = 45.dp, height = 9.dp)
            }
        }
    }

    @Composable
    fun FileItem() {
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.fillMaxWidth()
        ) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Block(width = 30.dp, height = 30.dp)
                Block(width = 138.dp, height = 17.dp)
            }
            Block(width = 16.dp, height = 16.dp)
        }
    }


    Column(
        verticalArrangement = Arrangement.spacedBy(12.dp),
        modifier = Modifier.padding(28.dp, 28.dp)
    ) {
        Block(
            width = 144.dp,
            height = 17.dp
        )
        FolderItem()
        FileItem()
        FileItem()
    }
}

@Composable
private fun ImportEntry(
    entry: VCurrentStorageEntry,
    onLocateEntry: (path: String) -> Unit
) {
    val painter = when (entry.entryTyp) {
        StorageEntryType.FOLDER -> painterResource(id = R.drawable.icon_folder)
        StorageEntryType.IMAGE -> painterResource(id = R.drawable.icon_image)
        StorageEntryType.MUSIC -> painterResource(id = R.drawable.icon_music_note)
        else -> painterResource(id = R.drawable.icon_file)
    }
    val onClick = {
        onLocateEntry(entry.path);
    }

    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
        modifier = Modifier
            .clickable {
                onClick()
            }
            .padding(0.dp, 8.dp)
            .fillMaxWidth()
    ) {
        Row(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.weight(1.0F)
        ) {
            Image(
                painter = painter,
                contentDescription = null,
                modifier = Modifier
                    .size(24.dp)
            )
            Text(
                text = entry.name,
                fontSize = 14.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
        }
        Box(modifier = Modifier.width(12.dp))
        Box(
            modifier = Modifier
                .size(16.dp)
        ) {
            if (entry.canCheck) {
                EaseCheckbox(
                    value = entry.checked,
                    onChange = {
                        onClick()
                    }
                )
            }
        }
    }
}

@Composable
private fun ImportEntries(
    selectedCount: Int,
    splitPaths: List<VSplitPathItem>,
    entries: List<VCurrentStorageEntry>,
    onPopRoute: () -> Unit,
) {
    @Composable
    fun PathTab(
        text: String,
        path: String,
        disabled: Boolean,
    ) {
        val color = if (!disabled) {
            MaterialTheme.colorScheme.onSurface
        } else {
            MaterialTheme.colorScheme.surfaceVariant
        }
        Text(
            text = text,
            color = color,
            fontSize = 10.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier
                .clickable(
                    enabled = !disabled,
                    onClick = {
                        Bridge.dispatchClick(StorageImportWidget.FolderNav(path))
                    }
                )
                .clip(RoundedCornerShape(2.dp))
                .widthIn(10.dp, 100.dp)
                .padding(4.dp, 2.dp)
        )
    }
    Box(
        modifier = Modifier.fillMaxSize()
    ) {
        Column {
            Row(
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .wrapContentHeight()
                    .padding(28.dp, 8.dp)
                    .horizontalScroll(rememberScrollState())
            ) {
                PathTab(
                    text = stringResource(id = R.string.import_musics_paths_root),
                    path = "/",
                    disabled = splitPaths.isEmpty()
                )
                for ((index, v) in splitPaths.withIndex()) {
                    Text(
                        text = ">",
                        fontSize = 10.sp,
                    )
                    PathTab(
                        text = v.name,
                        path = v.path,
                        disabled = index == splitPaths.size - 1,
                    )
                }
            }
            Column(
                modifier = Modifier
                    .padding(28.dp, 0.dp)
                    .verticalScroll(rememberScrollState())
            ) {
                for (entry in entries) {
                    ImportEntry(
                        entry = entry,
                        onLocateEntry = { path -> Bridge.dispatchClick(StorageImportWidget.StorageEntry(path)) },
                    )
                }
                Box(modifier = Modifier.height(12.dp))
            }
        }
        if (selectedCount > 0) {
            FloatingActionButton(
                containerColor = MaterialTheme.colorScheme.primary,
                contentColor = MaterialTheme.colorScheme.surface,
                onClick = {
                    Bridge.dispatchClick(StorageImportWidget.Import);
                    onPopRoute()
                },
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .offset((-40).dp, (-40).dp)
            ) {
                Icon(
                    painter = painterResource(id = R.drawable.icon_yes),
                    contentDescription = null,
                )
            }
        }
    }
}

@Composable
private fun ImportStorages(
    storageItems: List<VCurrentStorageEntriesStateStorageItem>
) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        modifier = Modifier
            .padding(28.dp, 0.dp)
            .horizontalScroll(rememberScrollState())
    ) {
        for (item in storageItems) {
            val bgColor = if (item.selected) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.surfaceVariant
            }
            val textColor = if (item.selected) {
                MaterialTheme.colorScheme.surface
            } else {
                Color.Unspecified
            }

            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(10.dp))
                    .clickable {
                        Bridge.dispatchClick(StorageImportWidget.StorageItem(item.id))
                    }
                    .background(bgColor)
                    .width(142.dp)
                    .height(65.dp)
            ) {
                Column(
                    modifier = Modifier
                        .padding(16.dp, 16.dp)
                ) {
                    Text(
                        text = item.name,
                        color = textColor,
                        fontSize = 14.sp,
                        lineHeight = 14.sp,
                    )
                    Text(
                        text = item.subtitle,
                        color = textColor,
                        fontSize = 10.sp,
                        lineHeight = 10.sp,
                    )
                }
                if (!item.isLocal) {
                    Icon(
                        painter = painterResource(id = R.drawable.icon_cloud),
                        contentDescription = null,
                        tint = Color.Black.copy(0.2F),
                        modifier = Modifier
                            .align(Alignment.BottomEnd)
                            .width(27.dp)
                            .offset(7.dp, 1.dp)
                    )
                }
            }
        }
    }
}

@Composable
private fun ImportMusicsWarningImpl(
    title: String,
    subTitle: String,
    color: Color,
    iconPainter: Painter,
    onClick: () -> Unit,
) {
    Box(
        contentAlignment = Alignment.Center,
        modifier = Modifier.fillMaxSize()
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier
                .clip(RoundedCornerShape(4.dp))
                .clickable {
                    onClick()
                }
                .padding(10.dp)
        ) {
            Box(modifier = Modifier
                .size(60.dp)
                .clip(RoundedCornerShape(999.dp))
                .background(color)
            ) {
                Icon(
                    painter = iconPainter,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.surface,
                    modifier = Modifier
                        .align(Alignment.Center)
                )
            }
            Text(
                text = title,
                fontSize = 14.sp,
                color = color,
            )
            Text(
                text = subTitle,
                fontSize = 12.sp,
                modifier = Modifier
                    .widthIn(0.dp, 220.dp)
            )
        }
    }
}

@Composable
private fun ImportMusicsError(
    type: CurrentStorageStateType,
) {
    val title = when (type) {
        CurrentStorageStateType.AUTHENTICATION_FAILED -> stringResource(id = R.string.import_musics_error_authentication_title)
        CurrentStorageStateType.TIMEOUT -> stringResource(id = R.string.import_musics_error_timeout_title)
        CurrentStorageStateType.UNKNOWN_ERROR -> stringResource(id = R.string.import_musics_error_unknown_title)
        else -> {
            throw RuntimeException("unsupported type")
        }
    }
    val desc = when (type) {
        CurrentStorageStateType.AUTHENTICATION_FAILED -> stringResource(id = R.string.import_musics_error_authentication_desc)
        CurrentStorageStateType.TIMEOUT -> stringResource(id = R.string.import_musics_error_timeout_desc)
        CurrentStorageStateType.UNKNOWN_ERROR -> stringResource(id = R.string.import_musics_error_unknown_desc)
        else -> {
            throw RuntimeException("unsupported type")
        }
    }

    ImportMusicsWarningImpl(
        title = title,
        subTitle = desc,
        color = MaterialTheme.colorScheme.error,
        iconPainter = painterResource(id = R.drawable.icon_warning),
        onClick = {
            Bridge.dispatchAction(ViewAction.StorageImport(StorageImportAction.RELOAD))
        }
    )
}

@Composable
fun ImportMusicsPage(
    vm: CurrentStorageEntriesViewModel
) {
    val navController = LocalNavController.current
    val state = vm.state.collectAsState().value
    val storageItems = state.storageItems.filter { item -> !item.isLocal }
    val titleText = when (state.selectedCount) {
        0 -> stringResource(id = R.string.import_musics_title_default)
        1 -> "${state.selectedCount} ${stringResource(id = R.string.import_musics_title_single_suffix)}"
        else -> "${state.selectedCount} ${stringResource(id = R.string.import_musics_title_multi_suffix)}"
    }
    fun doUndo() {
        if (!state.canUndo) {
            navController.popBackStack()
            return
        }

        Bridge.dispatchAction(ViewAction.StorageImport(StorageImportAction.UNDO));
    }
    fun popRoute() {
        navController.popBackStack()
    }

    BackHandler(enabled = state.canUndo) {
        doUndo()
    }
    Column(
        modifier = Modifier
            .fillMaxSize()
    ) {
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .padding(13.dp, 13.dp)
                .fillMaxWidth()
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
            ) {
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Default,
                    painter = painterResource(id = R.drawable.icon_back),
                    onClick = {
                        doUndo()
                    }
                )
                Text(
                    text = titleText
                )
            }
            Row {
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Default,
                    painter = painterResource(id = R.drawable.icon_toggle_all),
                    disabled = state.disabledToggleAll,
                    onClick = {
                        Bridge.dispatchClick(StorageImportWidget.ToggleAll)
                    }
                )
            }
        }
        ImportStorages(
            storageItems = storageItems
        )
        when (state.stateType) {
            CurrentStorageStateType.LOADING -> ImportEntriesSkeleton()
            CurrentStorageStateType.TIMEOUT, CurrentStorageStateType.AUTHENTICATION_FAILED, CurrentStorageStateType.UNKNOWN_ERROR -> ImportMusicsError(
                type = state.stateType,
            )
            else -> {
                ImportEntries(
                    selectedCount = state.selectedCount,
                    splitPaths = state.splitPaths,
                    entries = state.entries,
                    onPopRoute = { popRoute() }
                )
            }
        }
    }
}