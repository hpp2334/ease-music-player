package com.kutedev.easemusicplayer.widgets.musics

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.viewmodels.CurrentStorageEntriesViewModel
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import uniffi.ease_client.VCurrentStorageEntriesStateStorageItem

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
            .background(color)
            .clip(RoundedCornerShape(6.dp))
        )
    }

    @Composable
    fun FolderItem() {
        Row(
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Block(width = 30.dp, height = 30.dp)
            Column(
                verticalArrangement = Arrangement.SpaceBetween
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
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Block(width = 30.dp, height = 30.dp)
                Block(width = 138.dp, height = 17.dp)
            }
            Block(width = 16.dp, height = 16.dp)
        }
    }


    Column(
        verticalArrangement = Arrangement.spacedBy(12.dp)
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
private fun ImportStorages(
    storageItems: List<VCurrentStorageEntriesStateStorageItem>
) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        for (item in storageItems) {
            Column(
                modifier = Modifier
                    .padding(16.dp, 16.dp)
            ) {
                Text(text = item.name)
                Text(text = item.subtitle)
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

    state.storageItems[0]

    Column(
        modifier = Modifier
            .padding(13.dp, 13.dp)
            .fillMaxSize()
    ) {
        Row(
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Row {
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
                    onClick = {
                        navController.popBackStack()
                    }
                )
            }
        }
        ImportStorages(
            storageItems = state.storageItems
        )
        ImportEntriesSkeleton()
//        when (state.stateType) {
//            CurrentStorageStateType.LOADING -> ImportEntriesSkeleton()
//            else -> {}
//        }
    }
}