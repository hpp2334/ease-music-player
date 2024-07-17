package com.kutedev.easemusicplayer.widgets.playlist

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.ConfirmDialog
import com.kutedev.easemusicplayer.components.EaseContextMenu
import com.kutedev.easemusicplayer.components.EaseContextMenuItem
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.CurrentPlaylistViewModel
import uniffi.ease_client.PlaylistId
import uniffi.ease_client.removePlaylist
import java.util.Timer
import kotlin.concurrent.schedule

@Composable
private fun RemovePlaylistDialog(
    id: PlaylistId,
    title: String,
    open: Boolean,
    onClose: () -> Unit
) {
    val navController = LocalNavController.current

    ConfirmDialog(
        open = open,
        onConfirm = {
            onClose()
            navController.popBackStack()
            Timer("Remove playlist", false).schedule(0) {
                Bridge.invoke {
                    removePlaylist(id)
                }
            }
        },
        onCancel = onClose
    ) {
        Text(
            text = "${stringResource(id = R.string.playlist_remove_dialog_text)} “${title}”"
        )
    }
}

@Composable
fun PlaylistPage(vm: CurrentPlaylistViewModel) {
    val navController = LocalNavController.current
    val state = vm.state.collectAsState().value
    var moreMenuExpanded by remember { mutableStateOf(false) }
    var removeDialogOpen by remember { mutableStateOf(false) }
    val id = state.id

    val items = state.items

    val countSuffixStringId = if (items.size <= 1) {
        R.string.playlist_list_count_suffix
    } else {
        R.string.playlist_list_count_suffixes
    }

    if (id == null) {
        return
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
    ) {
        Column(

        ) {
            Box(
                modifier = Modifier
                    .height(157.dp)
                    .fillMaxWidth()
            ) {
                Image(
                    modifier = Modifier
                        .fillMaxSize(),
                    painter = painterResource(id = R.drawable.cover_default_playlist_image),
                    contentDescription = null,
                    contentScale = ContentScale.FillWidth
                )
                Row(
                    horizontalArrangement = Arrangement.SpaceBetween,
                    modifier = Modifier
                        .padding(13.dp, 13.dp)
                        .fillMaxWidth()
                ) {
                    EaseIconButton(
                        sizeType = EaseIconButtonSize.Medium,
                        buttonType = EaseIconButtonType.Surface,
                        painter = painterResource(id = R.drawable.icon_back),
                        onClick = {
                            navController.popBackStack()
                        }
                    )
                    Box {
                        EaseIconButton(
                            sizeType = EaseIconButtonSize.Medium,
                            buttonType = EaseIconButtonType.Surface,
                            painter = painterResource(id = R.drawable.icon_vertialcal_more),
                            onClick = { moreMenuExpanded = true; }
                        )
                        Box(
                            contentAlignment = Alignment.TopEnd,
                            modifier = Modifier
                                .offset(20.dp, (20).dp)
                        ) {
                            EaseContextMenu(
                                expanded = moreMenuExpanded,
                                onDismissRequest = { moreMenuExpanded = false; },
                                items = listOf(
                                    EaseContextMenuItem(
                                        stringId = R.string.playlist_context_menu_import,
                                        onClick = {

                                        }
                                    ),
                                    EaseContextMenuItem(
                                        stringId = R.string.playlist_context_menu_edit,
                                        onClick = {

                                        }
                                    ),
                                    EaseContextMenuItem(
                                        stringId = R.string.playlist_context_menu_remove,
                                        isError = true,
                                        onClick = {
                                            moreMenuExpanded = false;
                                            removeDialogOpen = true
                                        }
                                    ),
                                )
                            )
                        }
                    }
                }
                Column(
                    modifier = Modifier
                        .offset(48.dp, 60.dp)
                ) {
                    Text(
                        text = state.title,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.surface,
                        fontSize = 24.sp,
                    )
                    Text(
                        text = "${items.size} ${stringResource(id = countSuffixStringId)} · ${state.duration}",
                        color = MaterialTheme.colorScheme.surface,
                        fontSize = 14.sp,
                    )
                }
            }
            if (items.isEmpty()) {
                Box(
                    modifier = Modifier
                        .fillMaxSize(),
                    contentAlignment = Alignment.Center
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        modifier = Modifier
                            .clickable {

                            }
                            .clip(RoundedCornerShape(16.dp))
                            .padding(24.dp, 24.dp),
                    ) {
                        Image(
                            painter = painterResource(id = R.drawable.empty_playlist),
                            contentDescription = null,
                        )
                        Box(modifier = Modifier.height(11.dp))
                        Text(
                            text = stringResource(id = R.string.playlist_empty_list)
                        )
                    }
                }
            } else {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                ) {

                }
            }
        }
    }
    RemovePlaylistDialog(
        id = id,
        title = state.title,
        open = removeDialogOpen,
        onClose = { removeDialogOpen = false }
    )
}