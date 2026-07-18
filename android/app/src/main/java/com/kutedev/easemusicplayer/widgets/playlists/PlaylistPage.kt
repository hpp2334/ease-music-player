package com.kutedev.easemusicplayer.widgets.playlists

import EaseImage
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.ConfirmDialog
import com.kutedev.easemusicplayer.components.EaseContextMenu
import com.kutedev.easemusicplayer.components.EaseContextMenuItem
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonColors
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.customAnchoredDraggable
import com.kutedev.easemusicplayer.components.easeIconButtonSizeToDp
import com.kutedev.easemusicplayer.components.rememberCustomAnchoredDraggableState
import com.kutedev.easemusicplayer.viewmodels.PlayerVM
import com.kutedev.easemusicplayer.viewmodels.PlaylistVM
import com.kutedev.easemusicplayer.viewmodels.durationStr
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RouteImport
import com.kutedev.easemusicplayer.core.RouteMusicPlayer
import com.kutedev.easemusicplayer.singleton.RouteImportType
import com.kutedev.easemusicplayer.viewmodels.EditPlaylistVM
import com.kutedev.easemusicplayer.widgets.appbar.BottomBar
import com.kutedev.easemusicplayer.widgets.appbar.BottomBarSpacer
import sh.calvin.reorderable.ReorderableCollectionItemScope
import sh.calvin.reorderable.ReorderableItem
import sh.calvin.reorderable.ScrollMoveMode
import sh.calvin.reorderable.rememberReorderableLazyGridState
import sh.calvin.reorderable.rememberReorderableLazyListState
import uniffi.ease_client_schema.DataSourceKey
import uniffi.ease_client_backend.MusicAbstract
import uniffi.ease_client_schema.MusicId

@Composable
private fun RemovePlaylistDialog(
    playlistVM: PlaylistVM = hiltViewModel()
) {
    val navController = LocalNavController.current
    val open by playlistVM.removeModalOpen.collectAsState()
    val playlistAbstr by playlistVM.playlistAbstr.collectAsState()

    ConfirmDialog(
        open = open,
        onConfirm = {
            playlistVM.closeRemoveModal()
            playlistVM.remove()
            navController.popBackStack()
        },
        onCancel = {
            playlistVM.closeRemoveModal()
        }
    ) {
        Text(
            text = "${stringResource(id = R.string.playlist_remove_dialog_text)} “${playlistAbstr.meta.title}”"
        )
    }
}

@Composable
private fun PlaylistHeader(
    playlistVM: PlaylistVM = hiltViewModel(),
    editPlaylistVM: EditPlaylistVM = hiltViewModel()
) {
    val navController = LocalNavController.current
    val context = LocalContext.current
    val playlistAbstr by playlistVM.playlistAbstr.collectAsState()
    val musics by playlistVM.playlistMusics.collectAsState()
    val cover = playlistAbstr.meta.showCover
    val title = playlistAbstr.meta.title
    val duration = playlistAbstr.durationStr()

    var moreMenuExpanded by remember { mutableStateOf(false) }
    val countSuffixStringId = if (musics.size <= 1) {
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
                overrideColors = EaseIconButtonColors().copy(iconTint = Color.White),
                onClick = {
                    navController.popBackStack()
                }
            )
            Box {
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Surface,
                    overrideColors = EaseIconButtonColors().copy(iconTint = Color.White),
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
                                    playlistVM.prepareImportMusics(context)
                                    navController.navigate(RouteImport(RouteImportType.Music))
                                }
                            ),
                            EaseContextMenuItem(
                                stringId = R.string.playlist_context_menu_edit,
                                onClick = {
                                    editPlaylistVM.openModal()
                                }
                            ),
                            EaseContextMenuItem(
                                stringId = R.string.playlist_context_menu_remove,
                                isError = true,
                                onClick = {
                                    playlistVM.openRemoveModal()
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
                color = Color.White,
                fontSize = 24.sp,
                lineHeight = 26.sp,
                overflow = TextOverflow.Ellipsis,
                maxLines = 2
            )
            Text(
                text = "${musics.size} ${stringResource(id = countSuffixStringId)} · ${duration}",
                color = Color.White,
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
private fun ReorderableCollectionItemScope.PlaylistItem(
    item: MusicAbstract,
    index: Int,
    playing: Boolean,
    currentSwipingMusicId: MusicId?,
    onSwipe: () -> Unit,
    onRemove: () -> Unit,
    playlistVM: PlaylistVM = hiltViewModel(),
    playerVM: PlayerVM = hiltViewModel()
) {
    val navController = LocalNavController.current

    val density = LocalDensity.current
    val panelWidthDp = 48.dp

    val playlistAbstr by playlistVM.playlistAbstr.collectAsState()
    val id = item.meta.id
    val title = item.meta.title
    val duration = item.durationStr()

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
    val dragHandleColor = if (playing) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant
    }

    LaunchedEffect(currentSwipingMusicId) {
        if (currentSwipingMusicId != id) {
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
                    navController.navigate(RouteMusicPlayer())
                    playerVM.play(id, playlistAbstr.meta.id)
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
                Text(
                    modifier = Modifier.draggableHandle(),
                    text = (index + 1).toString(),
                    color = dragHandleColor,
                    maxLines = 1,
                    fontSize = 14.sp,
                )
                Text(
                    text = title,
                    color = color,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    fontSize = 14.sp,
                )
            }
            Box(modifier = Modifier.width(16.dp))
            Text(
                text = duration,
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
    scaffoldPadding: PaddingValues,
    playlistVM: PlaylistVM = hiltViewModel(),
    playerVM: PlayerVM = hiltViewModel()
) {
    var swipingMusicId by remember {
        mutableStateOf<MusicId?>(null)
    }
    val musics by playlistVM.playlistMusics.collectAsState()
    val currentPlaying by playerVM.music.collectAsState()

    val lazyListState = rememberLazyListState()
    val reorderableLazyListState =
        rememberReorderableLazyListState(lazyListState = lazyListState) { from, to ->
            playlistVM.musicMoveTo(from.index - 1, to.index - 1)
        }

    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .padding(0.dp),
        state = lazyListState
    ) {
        item {
            Box(modifier = Modifier.height(48.dp))
        }
        items(musics.size, key = { musics[it].meta.id.value }) {
            val item = musics[it];
            val id = item.meta.id
            val playing = id == currentPlaying?.meta?.id

            ReorderableItem(reorderableLazyListState, key = item.meta.id.value) { isDragging ->
                PlaylistItem(
                    item = item,
                    index = it,
                    playing = playing,
                    currentSwipingMusicId = swipingMusicId,
                    onSwipe = { swipingMusicId = id },
                    onRemove = {
                        if (swipingMusicId == id) {
                            swipingMusicId = null
                        }
                        playlistVM.removeMusic(id)
                    }
                )
            }
        }
        item {
            BottomBarSpacer(
                hasCurrentMusic = currentPlaying?.meta?.id != null,
                scaffoldPadding = scaffoldPadding,
            )
        }
    }
}

@Composable
fun PlaylistPage(
    playlistVM: PlaylistVM = hiltViewModel(),
    playerVM: PlayerVM = hiltViewModel(),
    scaffoldPadding: PaddingValues,
) {
    val navController = LocalNavController.current
    val musics by playlistVM.playlistMusics.collectAsState()
    val playlistAbstr by playlistVM.playlistAbstr.collectAsState()

    Box(
        modifier = Modifier
            .background(MaterialTheme.colorScheme.surface)
            .fillMaxSize()
    ) {
        Column {
            PlaylistHeader()
            if (musics.isEmpty()) {
                EmptyPlaylist()
            } else {
                PlaylistItemsBlock(
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
                disabled = musics.isEmpty(),
                onClick = {
                    val m = musics.firstOrNull()
                    if (m != null) {
                        navController.navigate(RouteMusicPlayer())
                        playerVM.play(m.meta.id, playlistAbstr.meta.id)
                    }
                }
            )
        }
        BottomBar(
            bottomBarPageState = null,
            scaffoldPadding = scaffoldPadding,
        )
    }
    RemovePlaylistDialog()
}