package com.kutedev.easemusicplayer.widgets.playlists

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupId
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.exp

/** Per-gesture phases of the adjust-mode input state machine. */
enum class AdjustPhase {
    Idle,
    /** Pointer down, undecided (tap / scroll / long-press-drag). */
    Pending,
    /** Finger-dragging the list (this controller owns scrolling in adjust mode). */
    Scrolling,
    /** Inertial scroll decay after lift. */
    Flinging,
    /** Long-press claimed a card / header drag. */
    Dragging,
}

internal enum class DragKind { None, Card, Header }

/**
 * Plain layout snapshot pushed by the Compose adapter (mapped from
 * `LazyGridState.layoutInfo`). Everything is in grid-viewport pixels —
 * the same coordinate space pointer events arrive in.
 */
data class GridSnapshot(
    val viewportTop: Float,
    val viewportBottom: Float,
    val viewportWidth: Float,
    val sections: List<SectionGeometry<PlaylistGroupId>>,
)

/** Everything the overlay / indicator layer renders for the live gesture. */
data class DragRender(
    val overlayTopLeft: Vec2,
    val overlaySize: Vec2,
    /** Slot-shaped indicator for a positioned card drop (null = bar). */
    val cardIndicator: RectF?,
    /** Full-width bar: header-zone drop or group-insertion boundary. */
    val barY: Float?,
    val isHeaderDrag: Boolean,
    val draggedCardId: PlaylistId?,
    val draggedGroupId: PlaylistGroupId?,
)

/**
 * Adjust-mode input controller: a single event-driven state machine that
 * owns ALL pointer behavior over the playlists grid while adjusting
 * (tap / scroll + fling / long-press drag), so nothing is left to
 * gesture arbitration with the grid's scrollable (`userScrollEnabled`
 * is false in adjust mode and the grid is covered by the gesture layer).
 *
 * The Compose layer is a thin adapter: it forwards pointer events
 * (`onDown`/`onMove`/`onUp`/`onCancel`), pushes layout snapshots
 * (`onLayout`, from `snapshotFlow { lazyGridState.layoutInfo }`) and a
 * frame clock (`onTick`), and executes the injected effects
 * (`scrollBy` / `haptic` / commits). All behavior is deterministic and
 * unit-testable on the JVM — the same resolver ([DragGeometry]) drives
 * the live indicator and the single commit on lift.
 */
class AdjustDragController(
    private val scope: CoroutineScope,
    private val scrollBy: suspend (Float) -> Unit,
    private val haptic: () -> Unit,
    private val commitCardMove: (
        playlistId: PlaylistId,
        originGroupId: PlaylistGroupId,
        targetGroupId: PlaylistGroupId,
        slot: Int,
    ) -> Unit,
    private val commitGroupMove: (
        draggedId: PlaylistGroupId,
        beforeGroupId: PlaylistGroupId?,
    ) -> Unit,
    private val snapshotProvider: () -> GridSnapshot?,
    private val config: Config = Config(),
) {
    data class Config(
        val longPressTimeoutMs: Long = 500L,
        val touchSlop: Float = 32f,
        val minFlingVelocityPxPerSec: Float = 50f,
        val flingDecayPerSec: Float = 2.0f,
        val autoScrollBandPx: Float = 120f,
        val autoScrollMinPxPerSec: Float = 300f,
        val autoScrollMaxPxPerSec: Float = 1800f,
    )

    internal data class State(
        val phase: AdjustPhase = AdjustPhase.Idle,
        val activePointer: Long = -1L,
        val downTimeMs: Long = 0L,
        val downPosition: Vec2 = Vec2.Zero,
        val pointer: Vec2 = Vec2.Zero,
        val drift: Vec2 = Vec2.Zero,
        val kind: DragKind = DragKind.None,
        val draggedCardId: PlaylistId? = null,
        val draggedGroupId: PlaylistGroupId? = null,
        val originGroupId: PlaylistGroupId? = null,
        /** Pointer offset within the dragged item at claim. */
        val pickup: Vec2 = Vec2.Zero,
        val cardSize: Vec2 = Vec2.Zero,
        /** Fling scroll velocity, px/sec (scroll direction). */
        val flingVelocity: Float = 0f,
    )

    internal var state by mutableStateOf(State())
        private set
    var lastLayout by mutableStateOf<GridSnapshot?>(null)
        private set
    private var lastTickNs = 0L
    private val tracker = VelocityWindow()

    /** The pointer id this gesture tracks (adapter filters events by it). */
    val activePointerId: Long get() = state.activePointer

    fun isCardDragged(id: PlaylistId): Boolean =
        state.phase == AdjustPhase.Dragging && state.draggedCardId == id

    fun isHeaderDragged(id: PlaylistGroupId): Boolean =
        state.phase == AdjustPhase.Dragging && state.draggedGroupId == id

    // ------------------------------------------------------------------
    // Events (in)
    // ------------------------------------------------------------------

    fun onLayout(snapshot: GridSnapshot) {
        lastLayout = snapshot
    }

    fun onDown(pointerId: Long, position: Vec2, timeMs: Long) {
        if (state.phase != AdjustPhase.Idle) {
            return
        }
        tracker.reset()
        tracker.add(timeMs, position.y)
        state = State(
            phase = AdjustPhase.Pending,
            activePointer = pointerId,
            downTimeMs = timeMs,
            downPosition = position,
            pointer = position,
        )
    }

    fun onMove(position: Vec2, timeMs: Long) {
        if (state.activePointer == -1L) {
            return
        }
        tracker.add(timeMs, position.y)
        when (state.phase) {
            AdjustPhase.Pending -> {
                // Displacement from the down position (NOT accumulated
                // path length): finger jitter during the long-press
                // hold must not trip the slop check — only a genuine
                // excursion past touch slop becomes a scroll.
                val drift = position - state.downPosition
                if (drift.getDistance() > config.touchSlop) {
                    state = state.copy(phase = AdjustPhase.Scrolling, pointer = position, drift = drift)
                } else {
                    state = state.copy(pointer = position, drift = drift)
                }
            }
            AdjustPhase.Scrolling -> {
                val delta = position.y - state.pointer.y
                state = state.copy(pointer = position)
                scroll(-delta)
            }
            AdjustPhase.Dragging -> {
                state = state.copy(pointer = position)
            }
            AdjustPhase.Idle, AdjustPhase.Flinging -> {}
        }
    }

    fun onUp() {
        when (state.phase) {
            AdjustPhase.Pending, AdjustPhase.Flinging -> reset()
            AdjustPhase.Scrolling -> {
                val velocity = tracker.velocityY()
                if (abs(velocity) > config.minFlingVelocityPxPerSec) {
                    state = state.copy(phase = AdjustPhase.Flinging, flingVelocity = -velocity)
                } else {
                    reset()
                }
            }
            AdjustPhase.Dragging -> finishDrag()
            AdjustPhase.Idle -> {}
        }
    }

    fun onCancel() = reset()

    /**
     * Frame clock: drives the long-press claim (Pending), fling decay
     * (Flinging) and drag auto-scroll (Dragging). `nowNs` is the frame
     * timestamp — the controller's single time source.
     */
    fun onTick(nowNs: Long) {
        val dtSec = if (lastTickNs == 0L) 0f else (nowNs - lastTickNs) / 1_000_000_000f
        lastTickNs = nowNs
        val nowMs = nowNs / 1_000_000L
        when (state.phase) {
            AdjustPhase.Pending -> {
                if (nowMs - state.downTimeMs >= config.longPressTimeoutMs) {
                    claimDrag()
                }
            }
            AdjustPhase.Flinging -> {
                val v = state.flingVelocity
                val next = v * exp(-config.flingDecayPerSec * dtSec)
                if (abs(next) <= config.minFlingVelocityPxPerSec) {
                    state = state.copy(phase = AdjustPhase.Idle, flingVelocity = 0f)
                } else {
                    state = state.copy(flingVelocity = next)
                }
                scroll(v * dtSec)
            }
            AdjustPhase.Dragging -> autoScrollTick(dtSec)
            AdjustPhase.Idle, AdjustPhase.Scrolling -> {}
        }
    }

    /** Aborts any live gesture (mode exit, layer disposal). No commit. */
    fun reset() {
        state = State()
    }

    // ------------------------------------------------------------------
    // Rendering (out) — read during composition; backed by snapshot state
    // ------------------------------------------------------------------

    fun render(): DragRender? {
        val s = state
        val snap = lastLayout
        if (s.phase != AdjustPhase.Dragging || snap == null) {
            return null
        }
        val overlayTopLeft = s.pointer - s.pickup
        return when (s.kind) {
            DragKind.Card -> {
                val target = resolveCardDrop(snap.sections, s.pointer)
                val section = target?.let { t -> snap.sections.firstOrNull { it.groupId == t.groupId } }
                val contentRight = snap.sections.flatMap { it.visibleCards }
                    .maxOfOrNull { it.rect.right } ?: snap.viewportWidth
                // Zone-aware indicator language: group-edge zones (top /
                // bottom of a group) get the thin bar; precise positions
                // and placeholder cells get the slot outline.
                var cardIndicator: RectF? = null
                var barY: Float? = null
                if (section != null && target != null) {
                    when (target.zone) {
                        DropZone.Header, DropZone.Head ->
                            barY = section.header?.bottom
                        DropZone.Position ->
                            if (section.expanded) {
                                cardIndicator = cardSlotIndicatorRect(section, target.slot, contentRight)
                            }
                        DropZone.End ->
                            if (section.expanded) {
                                // Placeholder cell: the empty trailing
                                // cell when the last row is partial,
                                // else the wrapped cell below.
                                cardIndicator = cardSlotIndicatorRect(
                                    section, section.memberCount, contentRight,
                                )
                            }
                    }
                    if (cardIndicator == null && barY == null) {
                        // Collapsed / empty sections: bar under header.
                        barY = section.header?.bottom
                    }
                }
                DragRender(
                    overlayTopLeft = overlayTopLeft,
                    overlaySize = s.cardSize,
                    cardIndicator = cardIndicator,
                    barY = barY,
                    isHeaderDrag = false,
                    draggedCardId = s.draggedCardId,
                    draggedGroupId = null,
                )
            }
            DragKind.Header -> {
                val others = snap.sections
                    .filter { it.groupId != s.draggedGroupId }
                    .mapNotNull { section -> section.header?.let { section.groupId to it } }
                val rects = others.map { it.second }
                val index = insertionIndexByCenterY(rects, s.pointer.y)
                DragRender(
                    overlayTopLeft = overlayTopLeft,
                    overlaySize = Vec2.Zero,
                    cardIndicator = null,
                    barY = insertionBarY(rects, index),
                    isHeaderDrag = true,
                    draggedCardId = null,
                    draggedGroupId = s.draggedGroupId,
                )
            }
            DragKind.None -> null
        }
    }

    // ------------------------------------------------------------------
    // Internals
    // ------------------------------------------------------------------

    private sealed interface Hit {
        data class Card(val groupId: PlaylistGroupId, val card: CardGeometry) : Hit
        data class Header(val groupId: PlaylistGroupId, val rect: RectF) : Hit
    }

    private fun hitTest(snap: GridSnapshot, p: Vec2): Hit? {
        for (section in snap.sections) {
            val header = section.header
            if (header != null && header.contains(p)) {
                return Hit.Header(section.groupId, header)
            }
            for (card in section.visibleCards) {
                if (card.rect.contains(p)) {
                    return Hit.Card(section.groupId, card)
                }
            }
        }
        return null
    }

    /** Long-press fired while Pending: claim a drag if an item is under the down position. */
    private fun claimDrag() {
        val snap = lastLayout ?: return
        when (val hit = hitTest(snap, state.downPosition)) {
            is Hit.Card -> {
                state = state.copy(
                    phase = AdjustPhase.Dragging,
                    kind = DragKind.Card,
                    draggedCardId = PlaylistId(hit.card.id),
                    originGroupId = hit.groupId,
                    pickup = state.downPosition - Vec2(hit.card.rect.left, hit.card.rect.top),
                    cardSize = Vec2(hit.card.rect.width, hit.card.rect.height),
                )
                haptic()
            }
            is Hit.Header -> {
                state = state.copy(
                    phase = AdjustPhase.Dragging,
                    kind = DragKind.Header,
                    draggedGroupId = hit.groupId,
                    originGroupId = hit.groupId,
                    pickup = state.downPosition - Vec2(hit.rect.left, hit.rect.top),
                )
                haptic()
            }
            null -> {
                // Long-press on empty space: no claim; stay Pending
                // until lift (harmless no-op).
            }
        }
    }

    /** Drag lift: resolve the target against the FRESHEST layout, then fire exactly one commit. */
    private fun finishDrag() {
        val s = state
        state = State()
        val snap = snapshotProvider() ?: lastLayout ?: return
        when (s.kind) {
            DragKind.Card -> {
                val dragged = s.draggedCardId ?: return
                val origin = s.originGroupId ?: return
                val target = resolveCardDrop(snap.sections, s.pointer) ?: return
                commitCardMove(dragged, origin, target.groupId, target.slot)
            }
            DragKind.Header -> {
                val dragged = s.draggedGroupId ?: return
                val others = snap.sections
                    .filter { it.groupId != dragged }
                    .mapNotNull { section -> section.header?.let { section.groupId to it } }
                val index = insertionIndexByCenterY(others.map { it.second }, s.pointer.y)
                commitGroupMove(dragged, others.getOrNull(index)?.first)
            }
            DragKind.None -> {}
        }
    }

    private fun autoScrollTick(dtSec: Float) {
        val snap = lastLayout ?: return
        val y = state.pointer.y
        val band = config.autoScrollBandPx
        val atTop = y < snap.viewportTop + band
        val atBottom = y > snap.viewportBottom - band
        if (!atTop && !atBottom) {
            return
        }
        val frac = if (atTop) {
            ((snap.viewportTop + band) - y) / band
        } else {
            (y - (snap.viewportBottom - band)) / band
        }.coerceIn(0f, 1f)
        val speed = config.autoScrollMinPxPerSec +
            (config.autoScrollMaxPxPerSec - config.autoScrollMinPxPerSec) * frac
        scroll(if (atTop) -speed * dtSec else speed * dtSec)
    }

    private fun scroll(dy: Float) {
        if (dy != 0f) {
            scope.launch { scrollBy(dy) }
        }
    }

    /** Minimal velocity estimator over a sliding time window (deterministic for tests). */
    private class VelocityWindow(private val windowMs: Long = 100L) {
        private val samples = ArrayList<Pair<Long, Float>>() // (timeMs, y)

        fun reset() = samples.clear()

        fun add(timeMs: Long, y: Float) {
            samples.add(timeMs to y)
            while (samples.size > 2 && timeMs - samples.first().first > windowMs) {
                samples.removeAt(0)
            }
        }

        fun velocityY(): Float {
            if (samples.size < 2) {
                return 0f
            }
            val first = samples.first()
            val last = samples.last()
            val dtSec = (last.first - first.first) / 1000f
            if (dtSec <= 0f) {
                return 0f
            }
            return (last.second - first.second) / dtSec
        }
    }
}
