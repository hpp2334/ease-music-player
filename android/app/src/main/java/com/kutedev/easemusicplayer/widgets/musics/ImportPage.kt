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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FloatingActionButton
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
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.viewmodel.compose.viewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseCheckbox
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.viewmodels.ImportVM
import com.kutedev.easemusicplayer.viewmodels.StoragesVM
import com.kutedev.easemusicplayer.viewmodels.VImportStorageEntry
import com.kutedev.easemusicplayer.viewmodels.entryTyp
import com.kutedev.easemusicplayer.core.LocalNavController
import uniffi.ease_client_backend.CurrentStorageStateType
import uniffi.ease_client_backend.StorageEntry
import uniffi.ease_client_backend.StorageEntryType

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
    entry: StorageEntry,
    checked: Boolean,
    allowTypes: List<StorageEntryType>,
    onClickEntry: (entry: StorageEntry) -> Unit
) {
    val entryTyp = entry.entryTyp()
    val canCheck = allowTypes.any({t -> t == entryTyp })
    val painter = when (entryTyp) {
        StorageEntryType.FOLDER -> painterResource(id = R.drawable.icon_folder)
        StorageEntryType.IMAGE -> painterResource(id = R.drawable.icon_image)
        StorageEntryType.MUSIC -> painterResource(id = R.drawable.icon_music_note)
        else -> painterResource(id = R.drawable.icon_file)
    }
    val onClick = {
        onClickEntry(entry);
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
            if (canCheck) {
                EaseCheckbox(
                    value = checked,
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
    importVM: ImportVM = hiltViewModel()
) {
    val navController = LocalNavController.current
    val splitPaths by importVM.splitPaths.collectAsState()
    val entries by importVM.entries.collectAsState()
    val selectedCount by importVM.selectedCount.collectAsState()
    val allowTypes by importVM.allowTypes.collectAsState()
    val selected by importVM.selected.collectAsState()

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
                        importVM.navigateDir(path)
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
            LazyColumn(
                modifier = Modifier
                    .padding(28.dp, 0.dp)
            ) {
                items(entries) {
                    ImportEntry(
                        entry = it,
                        checked = selected.contains(it.path),
                        allowTypes = allowTypes,
                        onClickEntry = { entry ->
                            importVM.clickEntry(entry)
                        },
                    )
                }
                item {
                    Box(modifier = Modifier.height(12.dp))
                }
            }
        }
        if (selectedCount > 0) {
            FloatingActionButton(
                containerColor = MaterialTheme.colorScheme.primary,
                contentColor = MaterialTheme.colorScheme.surface,
                onClick = {
                    navController.popBackStack()
                    importVM.finish()
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
    storagesVM: StoragesVM = hiltViewModel(),
    importVM: ImportVM = hiltViewModel()
) {
    val storageItems by storagesVM.storages.collectAsState()
    val selectedStorageId by importVM.selectedStorageId.collectAsState()

    Row(
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        modifier = Modifier
            .padding(28.dp, 0.dp)
            .horizontalScroll(rememberScrollState())
    ) {
        for (_item in storageItems) {
            val item = VImportStorageEntry(_item)

            val selected = selectedStorageId == item.id

            val bgColor = if (selected) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.surfaceVariant
            }
            val textColor = if (selected) {
                MaterialTheme.colorScheme.surface
            } else {
                Color.Unspecified
            }

            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(10.dp))
                    .clickable {
                        importVM.selectStorage(item.id)
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
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = item.subtitle,
                        color = textColor,
                        fontSize = 10.sp,
                        lineHeight = 10.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
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
    importVM: ImportVM = hiltViewModel()
) {
    val type by importVM.loadState.collectAsState()

    val title = when (type) {
        CurrentStorageStateType.AUTHENTICATION_FAILED -> stringResource(id = R.string.import_musics_error_authentication_title)
        CurrentStorageStateType.TIMEOUT -> stringResource(id = R.string.import_musics_error_timeout_title)
        CurrentStorageStateType.UNKNOWN_ERROR -> stringResource(id = R.string.import_musics_error_unknown_title)
        CurrentStorageStateType.NEED_PERMISSION -> stringResource(id = R.string.import_musics_error_permission_title)
        else -> {
            throw RuntimeException("unsupported type")
        }
    }
    val desc = when (type) {
        CurrentStorageStateType.AUTHENTICATION_FAILED -> stringResource(id = R.string.import_musics_error_authentication_desc)
        CurrentStorageStateType.TIMEOUT -> stringResource(id = R.string.import_musics_error_timeout_desc)
        CurrentStorageStateType.UNKNOWN_ERROR -> stringResource(id = R.string.import_musics_error_unknown_desc)
        CurrentStorageStateType.NEED_PERMISSION -> stringResource(id = R.string.import_musics_error_permission_desc)
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
            importVM.reload()
        }
    )
}

@Composable
fun ImportMusicsPage(
    importVM: ImportVM = hiltViewModel(),
    storagesVM: StoragesVM = hiltViewModel()
) {
    val storageItems by storagesVM.storages.collectAsState()
    val selectedCount by importVM.selectedCount.collectAsState()
    val canUndo by importVM.canUndo.collectAsState()
    val disabledToggleAll by importVM.disabledToggleAll.collectAsState()
    val loadState by importVM.loadState.collectAsState()

    val titleText = when (selectedCount) {
        0 -> stringResource(id = R.string.import_musics_title_default)
        1 -> "${selectedCount} ${stringResource(id = R.string.import_musics_title_single_suffix)}"
        else -> "${selectedCount} ${stringResource(id = R.string.import_musics_title_multi_suffix)}"
    }
    fun doUndo() {
        importVM.undo()
    }

    BackHandler(enabled = canUndo) {
        doUndo()
    }
    Column(
        modifier = Modifier
            .background(MaterialTheme.colorScheme.surface)
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
                    disabled = disabledToggleAll,
                    onClick = {
                        importVM.toggleAll()
                    }
                )
            }
        }
        ImportStorages()
        when (loadState) {
            CurrentStorageStateType.LOADING -> ImportEntriesSkeleton()
            CurrentStorageStateType.TIMEOUT,
            CurrentStorageStateType.AUTHENTICATION_FAILED,
            CurrentStorageStateType.UNKNOWN_ERROR,
            CurrentStorageStateType.NEED_PERMISSION -> ImportMusicsError()
            else -> {
                ImportEntries()
            }
        }
    }
}