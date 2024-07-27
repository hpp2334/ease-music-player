package com.kutedev.easemusicplayer.widgets.playlists

import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.Orientation
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
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.ExperimentalWearMaterialApi
import androidx.wear.compose.material.FractionalThreshold
import androidx.wear.compose.material.rememberSwipeableState
import androidx.wear.compose.material.swipeable
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.Routes
import com.kutedev.easemusicplayer.components.ConfirmDialog
import com.kutedev.easemusicplayer.components.EaseContextMenu
import com.kutedev.easemusicplayer.components.EaseContextMenuItem
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.CurrentMusicViewModel
import com.kutedev.easemusicplayer.viewmodels.CurrentPlaylistViewModel
import uniffi.ease_client.PlaylistId
import uniffi.ease_client.VCurrentMusicState
import uniffi.ease_client.VPlaylistMusicItem
import uniffi.ease_client.playMusic
import uniffi.ease_client.prepareImportEntriesInCurrentPlaylist
import uniffi.ease_client.removePlaylist
import java.util.Timer
import kotlin.concurrent.schedule
import kotlin.math.roundToInt

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
private fun PlaylistHeader(
    title: String,
    duration: String,
    items: List<VPlaylistMusicItem>,
    onRemoveDialogOpen: () -> Unit,
) {
    val navController = LocalNavController.current
    var moreMenuExpanded by remember { mutableStateOf(false) }
    val countSuffixStringId = if (items.size <= 1) {
        R.string.playlist_list_count_suffix
    } else {
        R.string.playlist_list_count_suffixes
    }

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
                                    Bridge.invoke {
                                        prepareImportEntriesInCurrentPlaylist()
                                    }
                                    navController.navigate(Routes.IMPORT_MUSICS)
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
                                    onRemoveDialogOpen()
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
                text = title,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.surface,
                fontSize = 24.sp,
            )
            Text(
                text = "${items.size} ${stringResource(id = countSuffixStringId)} · ${duration}",
                color = MaterialTheme.colorScheme.surface,
                fontSize = 14.sp,
            )
        }
    }
}

@Composable
private fun EmptyPlaylist() {
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
}

@OptIn(ExperimentalWearMaterialApi::class)
@Composable
private fun PlaylistItem(
    item: VPlaylistMusicItem,
    playing: Boolean,
    currentSwipingPlaylistId: PlaylistId?,
    onSwipe: () -> Unit,
) {
    val panelWidth = 48f
    val swipeableState = rememberSwipeableState(initialValue = 0)
    val anchors = mapOf(0f to 0, -panelWidth to 1) // Maps from position to state
    val swipeOffsetX = swipeableState.offset.value.roundToInt().dp

    LaunchedEffect(currentSwipingPlaylistId) {
        val isSwiping = currentSwipingPlaylistId == item.id
        if (!isSwiping && swipeableState.currentValue != 0) {
            swipeableState.animateTo(0, tween(300))
        }
    }

    val color = if (playing) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurface
    }
    val bgColor = if (playing) {
        MaterialTheme.colorScheme.secondary
    } else {
        MaterialTheme.colorScheme.surface
    }
    val durationColor = if (playing) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant
    }

    Box(
        modifier = Modifier
            .padding(
                end = 20.dp,
            )
            .clipToBounds()
            .fillMaxWidth()
    ) {
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .offset(swipeOffsetX, 0.dp)
                .swipeable(
                    state = swipeableState,
                    anchors = anchors,
                    thresholds = { _, _ -> FractionalThreshold(0.3f) },
                    orientation = Orientation.Horizontal
                )
                .padding(start = 20.dp)
                .clip(RoundedCornerShape(14.dp))
                .clickable {
                    Bridge.invoke {
                        playMusic(item.id)
                    }
                    onSwipe()
                }
                .background(bgColor)
                .padding(8.dp, 16.dp)
                .fillMaxWidth()
                .padding(6.dp, 0.dp)
        ){
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                modifier = Modifier.weight(1f)
            ) {
                Icon(
                    painter = painterResource(id = R.drawable.icon_music_note),
                    contentDescription = null,
                    tint = color,
                    modifier = Modifier
                        .size(24.dp)
                )
                Text(
                    text = item.title,
                    color = color,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    fontSize = 14.sp,
                )
            }
            Box(modifier = Modifier.width(16.dp))
            Text(
                text = item.duration,
                color = durationColor,
                maxLines = 1,
                modifier = Modifier.wrapContentWidth(),
                fontSize = 14.sp,
            )
        }
        Row(
            modifier = Modifier
                .offset(x = panelWidth.dp + swipeOffsetX)
                .align(alignment = Alignment.CenterEnd)
                .fillMaxHeight()
                .width(panelWidth.dp)
        ) {
            Box(modifier = Modifier.width(8.dp))
            EaseIconButton(
                sizeType = EaseIconButtonSize.Medium,
                buttonType = EaseIconButtonType.ErrorVariant,
                painter = painterResource(id = R.drawable.icon_deleteseep),
                onClick = {
                    Bridge.invoke {
                        removePlaylist(item.id)
                    }
                }
            )
        }
    }


    LaunchedEffect(swipeOffsetX) {
        if (!swipeableState.isAnimationRunning) {
            onSwipe()
        }
    }
}

@Composable
private fun PlaylistItemsBlock(
    items: List<VPlaylistMusicItem>,
    currentMusicState: VCurrentMusicState,
) {
    var swipingPlaylistId by remember {
        mutableStateOf<PlaylistId?>(null)
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
    ) {
        Box(modifier = Modifier.height(24.dp))
        for (item in items) {
            val playing = item.id == currentMusicState.id
            PlaylistItem(
                item = item,
                playing = playing,
                currentSwipingPlaylistId = swipingPlaylistId,
                onSwipe = {swipingPlaylistId = item.id}
            )
        }
        Box(modifier = Modifier.height(24.dp))
    }
}

@Composable
fun PlaylistPage(
    vm: CurrentPlaylistViewModel,
    currentMusicVM: CurrentMusicViewModel,
) {
    val navController = LocalNavController.current
    val state = vm.state.collectAsState().value
    val currentMusicState = currentMusicVM.state.collectAsState().value
    var removeDialogOpen by remember { mutableStateOf(false) }
    val id = state.id

    val items = state.items

    if (id == null) {
        return
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
    ) {
        Column {
            PlaylistHeader(
                title = state.title,
                duration = state.duration,
                items = items,
                onRemoveDialogOpen = {
                    removeDialogOpen = true
                },
            )
            if (items.isEmpty()) {
                EmptyPlaylist()
            } else {
                PlaylistItemsBlock(
                    items = items,
                    currentMusicState = currentMusicState,
                )
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