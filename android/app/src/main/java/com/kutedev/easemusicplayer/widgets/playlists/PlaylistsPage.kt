package com.kutedev.easemusicplayer.widgets.playlists

import EaseImage
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
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
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import androidx.compose.foundation.clickable
import com.kutedev.easemusicplayer.components.ConfirmDialog
import com.kutedev.easemusicplayer.components.EaseContextMenu
import com.kutedev.easemusicplayer.components.EaseContextMenuItem
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.components.SimpleFormText
import com.kutedev.easemusicplayer.viewmodels.CreatePlaylistVM
import com.kutedev.easemusicplayer.viewmodels.PlaylistsVM
import com.kutedev.easemusicplayer.viewmodels.PlaylistsMode
import com.kutedev.easemusicplayer.viewmodels.PlaylistGridEntry
import com.kutedev.easemusicplayer.viewmodels.buildPlaylistGridEntries
import com.kutedev.easemusicplayer.viewmodels.durationStr
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RoutePlaylist
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.lazy.grid.LazyGridLayoutInfo
import androidx.compose.foundation.layout.offset
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.changedToUpIgnoreConsumed
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.unit.IntOffset
import kotlin.math.roundToInt
import android.view.HapticFeedbackConstants
import com.kutedev.easemusicplayer.singleton.types.PlaylistAbstract
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupId
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupMeta

/** Which group dialog is open on the playlists page. */
private sealed interface GroupDialog {
    data object Create : GroupDialog
    data class Rename(val group: PlaylistGroupMeta) : GroupDialog
    data class Delete(val group: PlaylistGroupMeta) : GroupDialog
}

/** Exclusive-choice radio dot (same look as the dialogs' pickers). */
@Composable
private fun GroupRadioDot(selected: Boolean, isError: Boolean) {
    val borderColor = if (isError) {
        MaterialTheme.colorScheme.error
    } else if (selected) {
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
                    .background(if (isError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary)
            )
        }
    }
}

/** One selectable row of the delete-group mode picker. */
@Composable
private fun DeleteGroupOptionRow(
    text: String,
    selected: Boolean,
    isError: Boolean = false,
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
        GroupRadioDot(selected = selected, isError = isError)
        Text(
            text = text,
            fontSize = 12.sp,
            color = if (isError) MaterialTheme.colorScheme.error else Color.Unspecified,
        )
    }
}

@Composable
fun PlaylistsSubpage(
    playlistsVM: PlaylistsVM = hiltViewModel(),
    createPlaylistVM: CreatePlaylistVM = hiltViewModel()
) {
    val playlists by playlistsVM.playlists.collectAsState()
    val groups by playlistsVM.groups.collectAsState()
    val sections by playlistsVM.sections.collectAsState()
    val playlistsMode by playlistsVM.mode.collectAsState()

    var groupDialog by remember { mutableStateOf<GroupDialog?>(null) }
    var plusMenuOpen by remember { mutableStateOf(false) }

    when {
        // Groups are still loading (the ensure-default self-heal runs
        // inside the first reload) — nothing to render yet.
        groups.isEmpty() -> {}
        playlists.isEmpty() -> {
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier.fillMaxSize()
            ) {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    modifier = Modifier
                        .clickable {
                            createPlaylistVM.openModal()
                        }
                        .clip(RoundedCornerShape(16.dp))
                        .padding(24.dp, 24.dp),
                ) {
                    Image(painter = painterResource(id = R.drawable.empty_playlists), contentDescription = null)
                    Box(modifier = Modifier.height(20.dp))
                    Text(
                        text = stringResource(id = R.string.playlist_empty),
                    )
                }
            }
        }
        else -> {
            Box {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                ) {
                    Row(
                        modifier = Modifier
                            .padding(24.dp, 8.dp)
                            .fillMaxWidth(),
                        horizontalArrangement = Arrangement.End
                    ) {
                        EaseIconButton(
                            sizeType = EaseIconButtonSize.Medium,
                            buttonType = EaseIconButtonType.Default,
                            painter = painterResource(id = R.drawable.icon_adjust),
                            disabled = playlistsMode == PlaylistsMode.Adjust,
                            onClick = {
                                playlistsVM.toggleMode()
                            }
                        )
                        Box {
                            EaseIconButton(
                                sizeType = EaseIconButtonSize.Medium,
                                buttonType = EaseIconButtonType.Default,
                                painter = painterResource(id = R.drawable.icon_plus),
                                disabled = playlistsMode == PlaylistsMode.Adjust,
                                onClick = {
                                    plusMenuOpen = true
                                }
                            )
                            EaseContextMenu(
                                expanded = plusMenuOpen,
                                onDismissRequest = { plusMenuOpen = false },
                                items = listOf(
                                    EaseContextMenuItem(
                                        stringId = R.string.playlists_menu_new_playlist,
                                        onClick = { createPlaylistVM.openModal() },
                                    ),
                                    EaseContextMenuItem(
                                        stringId = R.string.playlists_menu_new_group,
                                        onClick = { groupDialog = GroupDialog.Create },
                                    ),
                                ),
                            )
                        }
                    }
                    GroupedGridPlaylists(
                        sections = sections,
                        playlistsVM = playlistsVM,
                        onRenameGroup = { group -> groupDialog = GroupDialog.Rename(group) },
                        onDeleteGroup = { group -> groupDialog = GroupDialog.Delete(group) },
                    )
                }
                if (playlistsMode == PlaylistsMode.Adjust) {
                    FloatingActionButton(
                        containerColor = MaterialTheme.colorScheme.primary,
                        modifier = Modifier
                            .align(Alignment.BottomEnd)
                            .padding(32.dp),
                        onClick = {
                            playlistsVM.setMode(PlaylistsMode.Normal)
                        }
                    ) {
                        Icon(
                            painter = painterResource(id = R.drawable.icon_yes),
                            tint = Color.White,
                            contentDescription = null,
                        )
                    }
                }
            }
        }
    }

    when (val dialog = groupDialog) {
        is GroupDialog.Create -> {
            PlaylistGroupDialog(
                title = stringResource(R.string.playlist_group_new),
                initialName = "",
                onConfirm = { name ->
                    playlistsVM.createGroup(name)
                    groupDialog = null
                },
                onDismiss = { groupDialog = null },
            )
        }
        is GroupDialog.Rename -> {
            PlaylistGroupDialog(
                title = stringResource(R.string.playlist_group_rename),
                initialName = dialog.group.title,
                onConfirm = { name ->
                    playlistsVM.renameGroup(dialog.group, name)
                    groupDialog = null
                },
                onDismiss = { groupDialog = null },
            )
        }
        is GroupDialog.Delete -> {
            // Which removal mode the confirm applies: sweep playlists
            // into the first group (default, safe) or delete them with
            // the group (full cascade).
            var deletePlaylistsToo by remember { mutableStateOf(false) }

            ConfirmDialog(
                open = true,
                onConfirm = {
                    playlistsVM.removeGroup(dialog.group, deletePlaylistsToo)
                    groupDialog = null
                },
                onCancel = { groupDialog = null },
            ) {
                Column {
                    Text(
                        text = stringResource(
                            R.string.playlist_group_delete_dialog_text,
                            dialog.group.title,
                        ),
                        fontSize = 12.sp,
                    )
                    Box(modifier = Modifier.height(8.dp))
                    DeleteGroupOptionRow(
                        text = stringResource(R.string.playlist_group_delete_option_only),
                        selected = !deletePlaylistsToo,
                        onClick = { deletePlaylistsToo = false },
                    )
                    DeleteGroupOptionRow(
                        text = stringResource(R.string.playlist_group_delete_option_with_playlists),
                        selected = deletePlaylistsToo,
                        isError = true,
                        onClick = { deletePlaylistsToo = true },
                    )
                }
            }
        }
        null -> {}
    }
}

/** Simple create / rename dialog for playlist groups. */
@Composable
private fun PlaylistGroupDialog(
    title: String,
    initialName: String,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var name by remember { mutableStateOf(initialName) }

    Dialog(onDismissRequest = onDismiss) {
        Column(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(MaterialTheme.colorScheme.surface)
                .padding(24.dp, 24.dp),
        ) {
            Text(text = title)
            Box(modifier = Modifier.height(8.dp))
            SimpleFormText(
                label = null,
                value = name,
                onChange = { value -> name = value },
            )
            Row(
                horizontalArrangement = Arrangement.End,
                modifier = Modifier.fillMaxWidth(),
            ) {
                EaseTextButton(
                    text = stringResource(id = R.string.playlists_dialog_button_cancel),
                    type = EaseTextButtonType.Primary,
                    size = EaseTextButtonSize.Medium,
                    onClick = onDismiss,
                )
                EaseTextButton(
                    text = stringResource(id = R.string.playlists_dialog_button_ok),
                    type = EaseTextButtonType.Primary,
                    size = EaseTextButtonSize.Medium,
                    disabled = name.isBlank(),
                    onClick = { onConfirm(name.trim()) },
                )
            }
        }
    }
}

/**
 * The grouped playlists grid with a fully custom drag system (see
 * [PlaylistDragController]): the drag never mutates the data — the
 * drop target is resolved from pointer geometry against real layout
 * bounds ([DragGeometry]), shown as an insertion indicator, and
 * committed exactly once when the gesture ends.
 */
@Composable
private fun GroupedGridPlaylists(
    sections: List<com.kutedev.easemusicplayer.viewmodels.PlaylistGroupSection>,
    playlistsVM: PlaylistsVM,
    onRenameGroup: (PlaylistGroupMeta) -> Unit,
    onDeleteGroup: (PlaylistGroupMeta) -> Unit,
) {
    val entries = remember(sections) { buildPlaylistGridEntries(sections) }
    val mode by playlistsVM.mode.collectAsState()
    val lazyGridState = rememberLazyGridState()
    val currentSections by rememberUpdatedState(sections)
    val view = LocalView.current
    val viewConfiguration = LocalViewConfiguration.current
    val pageScope = rememberCoroutineScope()
    val controller = remember {
        AdjustDragController(
            scope = pageScope,
            scrollBy = { dy -> lazyGridState.scrollBy(dy) },
            haptic = { view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS) },
            commitCardMove = { id, from, to, slot ->
                playlistsVM.commitCardMove(id, from, to, slot)
            },
            commitGroupMove = { id, before -> playlistsVM.commitGroupMove(id, before) },
            snapshotProvider = { lazyGridState.layoutInfo.toGridSnapshot(currentSections) },
            config = AdjustDragController.Config(
                touchSlop = viewConfiguration.touchSlop,
                longPressTimeoutMs = viewConfiguration.longPressTimeoutMillis,
            ),
        )
    }

    // Adapter lifelines (adjust mode only): layout snapshots, frame
    // clock, and a hard reset when the adjust layer goes away.
    if (mode == PlaylistsMode.Adjust) {
        LaunchedEffect(controller) {
            snapshotFlow { lazyGridState.layoutInfo.toGridSnapshot(currentSections) }
                .collect { controller.onLayout(it) }
        }
        LaunchedEffect(controller) {
            while (true) {
                withFrameNanos { controller.onTick(it) }
            }
        }
        DisposableEffect(controller) {
            onDispose { controller.reset() }
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        LazyVerticalGrid(
            modifier = Modifier.fillMaxSize(),
            columns = GridCells.FixedSize(172.dp),
            horizontalArrangement = Arrangement.Center,
            state = lazyGridState,
            // In adjust mode the controller owns scrolling (the gesture
            // layer above forwards pointer events to it); the grid's
            // own gesture scroll is disabled — programmatic scrollBy
            // still works (auto-scroll / fling).
            userScrollEnabled = mode != PlaylistsMode.Adjust,
        ) {
            items(
                count = entries.size,
                key = { index -> entries[index].key },
                span = { index ->
                    if (entries[index] is PlaylistGridEntry.Header) {
                        GridItemSpan(maxLineSpan)
                    } else {
                        GridItemSpan(1)
                    }
                },
            ) { index ->
                val entry = entries[index]
                when (entry) {
                    is PlaylistGridEntry.Header -> PlaylistGroupHeader(
                        group = entry.group,
                        sectionPlaylistCount = sections
                            .firstOrNull { it.group.id == entry.group.id }
                            ?.playlists
                            ?.size ?: 0,
                        isLastGroup = sections.size <= 1,
                        isDragged = controller.isHeaderDragged(entry.group.id),
                        playlistsVM = playlistsVM,
                        onRename = { onRenameGroup(entry.group) },
                        onDelete = { onDeleteGroup(entry.group) },
                    )
                    is PlaylistGridEntry.Card -> {
                        // The drag gesture lives on the gesture layer
                        // above; the item itself only renders its
                        // ghosted placeholder while dragged.
                        Box(
                            modifier = Modifier.alpha(
                                if (controller.isCardDragged(entry.playlist.meta.id)) {
                                    0.15f
                                } else {
                                    1f
                                }
                            ),
                        ) {
                            PlaylistItem(playlist = entry.playlist, playlistsVM = playlistsVM)
                        }
                    }
                }
            }
        }

        // Gesture layer (adjust mode only): a transparent sibling ABOVE
        // the grid that forwards pointer events to the controller. The
        // controller owns scrolling (the grid's gesture scroll is
        // disabled), so this layer both wins hit-testing and is never
        // disposed — the item-disposal, consumption-race and
        // scroll-occlusion failure classes all disappear.
        if (mode == PlaylistsMode.Adjust) {
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .pointerInput(controller) {
                        awaitEachGesture {
                            val down = awaitFirstDown(requireUnconsumed = false)
                            controller.onDown(
                                down.id.value,
                                Vec2(down.position.x, down.position.y),
                                down.uptimeMillis,
                            )
                            var active = true
                            while (active) {
                                val event = awaitPointerEvent()
                                val change = event.changes.firstOrNull {
                                    it.id.value == controller.activePointerId
                                }
                                if (change != null) {
                                    if (change.changedToUpIgnoreConsumed() || !change.pressed) {
                                        // A clean lift is an unconsumed
                                        // "up"; a system cancel surfaces
                                        // as a consumed one.
                                        if (change.isConsumed) {
                                            controller.onCancel()
                                        } else {
                                            controller.onUp()
                                        }
                                        active = false
                                    } else {
                                        controller.onMove(
                                            Vec2(change.position.x, change.position.y),
                                            change.uptimeMillis,
                                        )
                                        change.consume()
                                    }
                                } else if (event.changes.all { !it.pressed }) {
                                    controller.onCancel()
                                    active = false
                                }
                            }
                        }
                    },
            )
        }

        // Drag overlay + drop indicators, rendered from the controller
        // state (its `render()` reads snapshot state — this recomposes
        // on every pointer move / layout change). The overlay Box
        // shares the grid's viewport coordinate space, so no
        // normalization is needed. Indicators are drawn AFTER the
        // floating card so the landing outline stays visible even when
        // the dragged card covers its own landing slot.
        val render = controller.render()
        if (render != null) {
            val density = LocalDensity.current
            with(density) {
                if (render.isHeaderDrag) {
                    val section = currentSections.firstOrNull {
                        it.group.id == render.draggedGroupId
                    }
                    val group = section?.group
                    if (group != null) {
                        Box(
                            modifier = Modifier
                                .offset {
                                    IntOffset(
                                        render.overlayTopLeft.x.roundToInt(),
                                        render.overlayTopLeft.y.roundToInt(),
                                    )
                                }
                                .shadow(8.dp, RoundedCornerShape(8.dp)),
                        ) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(6.dp),
                                modifier = Modifier
                                    .clip(RoundedCornerShape(8.dp))
                                    .background(MaterialTheme.colorScheme.surfaceVariant)
                                    .padding(horizontal = 12.dp, vertical = 8.dp),
                            ) {
                                Icon(
                                    painter = painterResource(id = R.drawable.icon_drag),
                                    contentDescription = null,
                                    tint = MaterialTheme.colorScheme.primary,
                                    modifier = Modifier.size(16.dp),
                                )
                                Text(
                                    text = group.title,
                                    fontSize = 13.sp,
                                    fontWeight = FontWeight.Medium,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    text = "${section.playlists.size}",
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Light,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                } else {
                    val playlist = currentSections
                        .flatMap { it.playlists }
                        .firstOrNull { it.meta.id == render.draggedCardId }
                    if (playlist != null) {
                        Box(
                            modifier = Modifier
                                .offset {
                                    IntOffset(
                                        render.overlayTopLeft.x.roundToInt(),
                                        render.overlayTopLeft.y.roundToInt(),
                                    )
                                }
                                .graphicsLayer {
                                    scaleX = 1.04f
                                    scaleY = 1.04f
                                }
                                .shadow(16.dp, RoundedCornerShape(8.dp))
                                .size(render.overlaySize.x.toDp(), render.overlaySize.y.toDp())
                                .clip(RoundedCornerShape(4.dp))
                                .background(MaterialTheme.colorScheme.surface),
                        ) {
                            PlaylistCardContent(playlist = playlist)
                        }
                    }
                }

                // Indicators on top of the floating card — the landing
                // outline must stay visible even when the dragged card
                // covers its own landing slot.
                render.cardIndicator?.let { rect ->
                    Box(
                        modifier = Modifier
                            .offset {
                                IntOffset(rect.left.roundToInt(), rect.top.roundToInt())
                            }
                            .size(rect.width.toDp(), rect.height.toDp())
                            .clip(RoundedCornerShape(20.dp))
                            .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.15f))
                            .border(2.dp, MaterialTheme.colorScheme.primary, RoundedCornerShape(20.dp)),
                    )
                }
                render.barY?.let { y ->
                    Box(
                        modifier = Modifier
                            .offset { IntOffset(0, y.roundToInt()) }
                            .fillMaxWidth()
                            .padding(horizontal = 8.dp)
                            .height(4.dp)
                            .clip(RoundedCornerShape(2.dp))
                            .background(MaterialTheme.colorScheme.primary),
                    )
                }
            }
        }
    }
}

/**
 * Map the grid's live layout into the controller's plain snapshot
 * (viewport pixels; item offsets are already in viewport coordinates —
 * the same space pointer events arrive in through the gesture layer).
 */
private fun LazyGridLayoutInfo.toGridSnapshot(
    sections: List<com.kutedev.easemusicplayer.viewmodels.PlaylistGroupSection>,
): GridSnapshot {
    val entryByKey = buildPlaylistGridEntries(sections).associateBy { it.key }
    val slotOf = HashMap<Long, Int>()
    sections.forEach { section ->
        section.playlists.forEachIndexed { index, playlist ->
            slotOf[playlist.meta.id.value] = index
        }
    }

    val headers = HashMap<PlaylistGroupId, RectF>()
    val cards = HashMap<PlaylistGroupId, MutableList<CardGeometry>>()
    for (info in visibleItemsInfo) {
        val entry = entryByKey[info.key] ?: continue
        val rect = RectF(
            info.offset.x.toFloat(),
            info.offset.y.toFloat(),
            (info.offset.x + info.size.width).toFloat(),
            (info.offset.y + info.size.height).toFloat(),
        )
        when (entry) {
            is PlaylistGridEntry.Header -> headers[entry.group.id] = rect
            is PlaylistGridEntry.Card -> cards.getOrPut(entry.group.id) { mutableListOf() }
                .add(
                    CardGeometry(
                        slot = slotOf[entry.playlist.meta.id.value] ?: 0,
                        rect = rect,
                        id = entry.playlist.meta.id.value,
                    )
                )
        }
    }
    return GridSnapshot(
        viewportTop = viewportStartOffset.toFloat(),
        viewportBottom = viewportEndOffset.toFloat(),
        viewportWidth = viewportSize.width.toFloat(),
        sections = sections.map { section ->
            SectionGeometry(
                groupId = section.group.id,
                header = headers[section.group.id],
                expanded = section.group.expanded,
                memberCount = section.playlists.size,
                visibleCards = cards[section.group.id] ?: emptyList(),
            )
        },
    )
}

/**
 * Full-span section header: chevron + group title + playlist count.
 * Tap toggles expand/collapse (persisted via `playlistGroup.setExpanded`);
 * long-press (or the ⋮ button) opens the rename / delete menu. In Adjust
 * mode the whole row is a long-press drag handle for reordering groups
 * (the gesture itself lives on the page's gesture layer — see
 * [AdjustDragController]).
 */
@Composable
private fun PlaylistGroupHeader(
    group: PlaylistGroupMeta,
    sectionPlaylistCount: Int,
    isLastGroup: Boolean,
    isDragged: Boolean,
    playlistsVM: PlaylistsVM,
    onRename: () -> Unit,
    onDelete: () -> Unit,
) {
    val mode by playlistsVM.mode.collectAsState()
    var menuOpen by remember { mutableStateOf(false) }
    val countSuffix = stringResource(R.string.playlist_group_count_suffix)

    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 8.dp, end = 8.dp)
            .then(
                if (mode == PlaylistsMode.Adjust) {
                    // No per-item gesture in adjust mode — dragging is
                    // handled (long-press) by the page container.
                    Modifier
                } else {
                    Modifier.pointerInput(group.id) {
                        detectTapGestures(
                            onTap = { playlistsVM.toggleExpanded(group.id) },
                            onLongPress = { menuOpen = true },
                        )
                    }
                }
            )
            .alpha(if (isDragged) 0.3f else 1f)
            .clip(RoundedCornerShape(8.dp))
            .padding(4.dp, 8.dp),
    ) {
        if (mode == PlaylistsMode.Adjust) {
            Icon(
                modifier = Modifier
                    .padding(end = 6.dp)
                    .size(16.dp),
                painter = painterResource(id = R.drawable.icon_drag),
                tint = MaterialTheme.colorScheme.primary,
                contentDescription = null,
            )
        }
        Icon(
            painter = painterResource(
                id = if (group.expanded) R.drawable.icon_collapse else R.drawable.icon_forward
            ),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(16.dp),
        )
        Text(
            text = group.title,
            fontSize = 13.sp,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier
                .padding(start = 6.dp)
                .weight(1.0F),
        )
        Text(
            text = "$sectionPlaylistCount $countSuffix",
            fontSize = 11.sp,
            fontWeight = FontWeight.Light,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Box {
            Icon(
                painter = painterResource(id = R.drawable.icon_vertialcal_more),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier
                    .padding(start = 8.dp)
                    .size(16.dp)
                    .clip(RoundedCornerShape(4.dp))
                    .clickable(enabled = mode != PlaylistsMode.Adjust) { menuOpen = true },
            )
            EaseContextMenu(
                expanded = menuOpen,
                onDismissRequest = { menuOpen = false },
                items = buildList {
                    add(
                        EaseContextMenuItem(
                            stringId = R.string.playlist_group_rename,
                            onClick = onRename,
                        )
                    )
                    // The last group can never be deleted (playlists
                    // must always live in a group).
                    if (!isLastGroup) {
                        add(
                            EaseContextMenuItem(
                                stringId = R.string.playlist_group_delete,
                                onClick = onDelete,
                                isError = true,
                            )
                        )
                    }
                },
            )
        }
    }
}

@Composable
private fun PlaylistItem(
    playlist: PlaylistAbstract,
    playlistsVM: PlaylistsVM,
) {
    val mode by playlistsVM.mode.collectAsState()
    val navController = LocalNavController.current

    Box(Modifier
        .then(if (mode == PlaylistsMode.Adjust) {
            // The drag gesture is attached by the grid (adjust mode);
            // cards are not clickable here.
            Modifier
        } else {
            Modifier.clickable(
                onClick = {
                    navController.navigate(RoutePlaylist(playlist.meta.id.value.toString()))
                },
            )
        })
    ) {
        PlaylistCardContent(playlist = playlist)
        if (mode == PlaylistsMode.Adjust) {
            Box(
                modifier = Modifier
                    .align(Alignment.TopStart)
                    .size(24.dp)
                    .clip(RoundedCornerShape(4.dp))
                    .background(MaterialTheme.colorScheme.primary),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    modifier = Modifier.size(12.dp),
                    painter = painterResource(id = R.drawable.icon_drag),
                    tint = Color.White,
                    contentDescription = null,
                )
            }
        }
    }
}

/** Card visuals (cover, title, meta) — reused by the drag overlay. */
@Composable
private fun PlaylistCardContent(playlist: PlaylistAbstract) {
    Column(
        modifier = Modifier.padding(12.dp, 8.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.Start,
    ) {
        Box(
            modifier = Modifier.clip(RoundedCornerShape(20.dp))
                .background(MaterialTheme.colorScheme.onSurfaceVariant).size(136.dp)
        ) {
            val cover = playlist.meta.showCover
            if (cover == null) {
                Image(
                    modifier = Modifier.fillMaxSize(),
                    painter = painterResource(id = R.drawable.cover_default_image),
                    contentDescription = null,
                    contentScale = ContentScale.FillWidth
                )
            } else {
                EaseImage(
                    modifier = Modifier.fillMaxSize(),
                    dataSourceKey = cover,
                    contentScale = ContentScale.FillWidth
                )
            }
        }
        Row(
            modifier = Modifier.padding(top = 8.dp)
        ) {
            Text(
                text = playlist.meta.title,
                fontSize = 14.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
        }
        Text(
            text = buildAnnotatedString {
                append("${playlist.musicCount} ${stringResource(id = R.string.music_count_unit)}")
                append("  ·  ")
                append(playlist.durationStr())
            },
            fontSize = 12.sp,
            fontWeight = FontWeight.Light,
            maxLines = 1,
        )
    }
}
