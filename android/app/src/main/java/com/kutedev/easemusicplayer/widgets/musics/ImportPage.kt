package com.kutedev.easemusicplayer.widgets.musics

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseCheckbox
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.CurrentStorageEntriesViewModel
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import uniffi.ease_client.CurrentStorageStateType
import uniffi.ease_client.StorageEntryType
import uniffi.ease_client.VCurrentStorageEntriesStateStorageItem
import uniffi.ease_client.VCurrentStorageEntry
import uniffi.ease_client.VSplitPathItem
import uniffi.ease_client.locateEntry
import uniffi.ease_client.selectEntry
import uniffi.ease_client.selectStorageInImport
import uniffi.ease_client.toggleAllCheckedEntries

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
    entry: VCurrentStorageEntry
) {
    val painter = when (entry.entryTyp) {
        StorageEntryType.FOLDER -> painterResource(id = R.drawable.icon_folder)
        StorageEntryType.IMAGE -> painterResource(id = R.drawable.icon_image)
        StorageEntryType.MUSIC -> painterResource(id = R.drawable.icon_music_note)
        else -> painterResource(id = R.drawable.icon_file)
    }

    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
        modifier = Modifier
            .clickable {
                if (entry.canCheck) {
                    Bridge.invoke {
                        selectEntry(entry.path)
                    }
                } else {
                    Bridge.invoke {
                        locateEntry(entry.path)
                    }
                }
            }
            .padding(0.dp, 8.dp)
            .fillMaxWidth()
    ) {
        Row(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
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
            )
        }
        if (entry.canCheck) {
            EaseCheckbox(
                value = entry.checked,
                onChange = {}
            )
        }
    }
}

@Composable
private fun ImportEntries(
    splitPaths: List<VSplitPathItem>,
    entries: List<VCurrentStorageEntry>
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
            MaterialTheme.colorScheme.onSurfaceVariant
        }
        Text(
            text = text,
            color = color,
            modifier = Modifier
                .clickable {
                    Bridge.invoke {
                        locateEntry(path)
                    }
                }
        )
    }

    Column {
        Row(
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .padding(28.dp, 0.dp)
                .offset((-14).dp)
        ) {
            PathTab(
                text = stringResource(id = R.string.import_musics_paths_root),
                path = "/",
                disabled = entries.isEmpty()
            )
            for ((index, v) in splitPaths.withIndex()) {
                Text(text = ">")
                PathTab(
                    text = v.name,
                    path = v.path,
                    disabled = index == entries.size - 1,
                )
            }
        }
        Column(
            modifier = Modifier
                .padding(28.dp, 0.dp)
        ) {
            for (entry in entries) {
                ImportEntry(
                    entry = entry
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
        modifier = Modifier.padding(28.dp, 0.dp)
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

            Column(
                modifier = Modifier
                    .clip(RoundedCornerShape(10.dp))
                    .clickable {
                        Bridge.invoke {
                            selectStorageInImport(item.id)
                        }
                    }
                    .background(bgColor)
                    .width(142.dp)
                    .height(65.dp)
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
        }
    }
}

@Composable
fun ImportMusicsPage(
    currentStorageEntriesVM: CurrentStorageEntriesViewModel
) {
    val navController = LocalNavController.current
    val state = currentStorageEntriesVM.state.collectAsState().value
    val titleText = when (state.selectedCount) {
        0 -> stringResource(id = R.string.import_musics_title_default)
        1 -> "${state.selectedCount} ${stringResource(id = R.string.import_musics_title_single_suffix)}"
        else -> "${state.selectedCount} ${stringResource(id = R.string.import_musics_title_multi_suffix)}"
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
                        navController.popBackStack()
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
                        Bridge.invoke {
                            toggleAllCheckedEntries()
                        }
                    }
                )
            }
        }
        ImportStorages(
            storageItems = state.storageItems
        )
        when (state.stateType) {
            CurrentStorageStateType.LOADING -> ImportEntriesSkeleton()
            else -> {
                ImportEntries(
                    splitPaths = state.splitPaths,
                    entries = state.entries
                )
            }
        }
    }
}