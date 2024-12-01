package com.kutedev.easemusicplayer.widgets.playlists

import EaseImage
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.DecayAnimationSpec
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.AnchoredDraggableState
import androidx.compose.foundation.gestures.DraggableState
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.anchoredDraggable
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.DeviceFontFamilyName
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.ConfirmDialog
import com.kutedev.easemusicplayer.components.CustomAnchoredDraggableState
import com.kutedev.easemusicplayer.components.EaseContextMenu
import com.kutedev.easemusicplayer.components.EaseContextMenuItem
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.customAnchoredDraggable
import com.kutedev.easemusicplayer.components.easeIconButtonSizeToDp
import com.kutedev.easemusicplayer.components.rememberCustomAnchoredDraggableState
import com.kutedev.easemusicplayer.core.UIBridgeController
import com.kutedev.easemusicplayer.viewmodels.EaseViewModel
import com.kutedev.easemusicplayer.widgets.appbar.BottomBar
import com.kutedev.easemusicplayer.widgets.appbar.BottomBarSpacer
import uniffi.ease_client.PlaylistDetailWidget
import uniffi.ease_client.RoutesKey
import uniffi.ease_client.VCurrentMusicState
import uniffi.ease_client.VPlaylistMusicItem
import uniffi.ease_client_shared.DataSourceKey
import uniffi.ease_client_shared.MusicId
import kotlin.math.roundToInt

@Composable
private fun RemovePlaylistDialog(
    title: String,
    open: Boolean,
    onClose: () -> Unit
) {
    val bridge = UIBridgeController.current
    ConfirmDialog(
        open = open,
        onConfirm = {
            onClose()
            bridge.popRoute()
            bridge.schedule {
                bridge.dispatchClick(PlaylistDetailWidget.Remove);
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
    cover: DataSourceKey?,
    title: String,
    duration: String,
    items: List<VPlaylistMusicItem>,
    onRemoveDialogOpen: () -> Unit,
) {
    val bridge = UIBridgeController.current
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
        if (cover == null) {
            Image(
                modifier = Modifier
                    .fillMaxSize(),
                painter = painterResource(id = R.drawable.cover_default_playlist_image),
                contentDescription = null,
                contentScale = ContentScale.FillWidth
            )
        } else {
            Box(
                modifier = Modifier.fillMaxSize()
            ) {
                EaseImage(
                    modifier = Modifier
                        .fillMaxSize(),
                    dataSourceKey = cover,
                    contentScale = ContentScale.FillWidth
                )
                Box(
                    modifier = Modifier
                        .background(Color.Black.copy(alpha = 0.6f))
                        .fillMaxSize()
                )
            }
        }
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
                    bridge.popRoute()
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
                                    bridge.dispatchClick(PlaylistDetailWidget.Import);
                                }
                            ),
                            EaseContextMenuItem(
                                stringId = R.string.playlist_context_menu_edit,
                                onClick = {
                                    bridge.dispatchClick(PlaylistDetailWidget.Edit);
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
                .padding(48.dp, 0.dp)
                .offset(0.dp, 60.dp)
        ) {
            Text(
                text = title,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.surface,
                fontSize = 24.sp,
                lineHeight = 26.sp,
                overflow = TextOverflow.Ellipsis,
                maxLines = 2
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

@Composable
private fun PlaylistItem(
    item: VPlaylistMusicItem,
    playing: Boolean,
    currentSwipingMusicId: MusicId?,
    onSwipe: () -> Unit,
    onRemove: () -> Unit,
) {
    val panelWidthDp = 48.dp

    val bridge = UIBridgeController.current
    val density = LocalDensity.current

    val anchoredDraggableState = with(density) {
        rememberCustomAnchoredDraggableState(
            initialValue = 0f,
            anchors = mapOf(
                0.dp.toPx() to "START",
                -panelWidthDp.toPx() to "END"
            ),
            animationSpec = tween(200),
        )
    }

    val color = if (playing) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurface
    }
    val bgColor = if (playing) {
        MaterialTheme.colorScheme.secondary
    } else {
        Color.Transparent
    }
    val durationColor = if (playing) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant
    }

    LaunchedEffect(currentSwipingMusicId) {
        if (currentSwipingMusicId != item.id) {
            anchoredDraggableState.animateTo(0f)
        }
    }

    Box(
        modifier = Modifier
            .padding(
                start = 20.dp,
                end = 20.dp,
            )
            .fillMaxWidth()
    ) {
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .offset(x = with(density) { anchoredDraggableState.value.toDp() })
                .clip(RoundedCornerShape(14.dp))
                .customAnchoredDraggable(
                    state = anchoredDraggableState,
                    orientation = Orientation.Horizontal,
                    onDragStarted = {
                        onSwipe()
                    }
                )
                .clickable {
                    bridge.dispatchClick(PlaylistDetailWidget.Music(item.id));
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
                fontFamily = FontFamily(Font(R.font.noto_sans)),
            )
        }
        Box(
            modifier = Modifier
                .clipToBounds()
                .fillMaxSize()
                .align(alignment = Alignment.CenterEnd)
        ) {
            Row(
                modifier = Modifier
                    .offset(x = panelWidthDp + with(density) { anchoredDraggableState.value.toDp() })
                    .fillMaxSize(),
                horizontalArrangement = Arrangement.End
            ) {
                Box(modifier = Modifier.width(8.dp))
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.ErrorVariant,
                    painter = painterResource(id = R.drawable.icon_deleteseep),
                    onClick = {
                        onRemove()
                    }
                )
            }
        }
    }
}

@Composable
private fun PlaylistItemsBlock(
    items: List<VPlaylistMusicItem>,
    currentMusicState: VCurrentMusicState,
    scaffoldPadding: PaddingValues,
) {
    val bridge = UIBridgeController.current
    var swipingMusicId by remember {
        mutableStateOf<MusicId?>(null)
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
    ) {
        Box(modifier = Modifier.height(48.dp))
        for (item in items) {
            val playing = item.id == currentMusicState.id

            key(item.id) {
                PlaylistItem(
                    item = item,
                    playing = playing,
                    currentSwipingMusicId = swipingMusicId,
                    onSwipe = {swipingMusicId = item.id},
                    onRemove = {
                        if (swipingMusicId == item.id) {
                            swipingMusicId = null
                        }
                        bridge.dispatchClick(PlaylistDetailWidget.RemoveMusic(item.id));
                    }
                )
            }
        }
        BottomBarSpacer(
            hasCurrentMusic = currentMusicState.id != null,
            scaffoldPadding = scaffoldPadding,
        )
    }
}

@Composable
fun PlaylistPage(
    evm: EaseViewModel,
    scaffoldPadding: PaddingValues,
) {
    val bridge = UIBridgeController.current
    val state = evm.currentPlaylistState.collectAsState().value
    val currentMusicState = evm.currentMusicState.collectAsState().value
    var removeDialogOpen by remember { mutableStateOf(false) }

    val items = state.items

    Box(
        modifier = Modifier
            .background(Color.White)
            .fillMaxSize()
    ) {
        Column {
            PlaylistHeader(
                cover = state.cover,
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
                    scaffoldPadding = scaffoldPadding,
                )
            }
        }
        Box(
            modifier = Modifier
                .align(Alignment.TopEnd)
                .offset((-20).dp, 157.dp - easeIconButtonSizeToDp(EaseIconButtonSize.Large) / 2)
        ) {
            EaseIconButton(
                sizeType = EaseIconButtonSize.Large,
                buttonType = EaseIconButtonType.Primary,
                painter = painterResource(id = R.drawable.icon_play),
                disabled = items.isEmpty(),
                onClick = {
                    bridge.dispatchClick(PlaylistDetailWidget.PlayAll);
                }
            )
        }
        BottomBar(
            currentRoute = RoutesKey.PLAYLIST,
            bottomBarPageState = null,
            evm = evm,
            scaffoldPadding = scaffoldPadding,
        )
    }
    RemovePlaylistDialog(
        title = state.title,
        open = removeDialogOpen,
        onClose = { removeDialogOpen = false }
    )
}