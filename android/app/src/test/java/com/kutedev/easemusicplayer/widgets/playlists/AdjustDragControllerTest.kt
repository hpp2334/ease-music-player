package com.kutedev.easemusicplayer.widgets.playlists

import com.kutedev.easemusicplayer.singleton.types.PlaylistGroupId
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Behavior spec of the adjust-mode input state machine. All gestures
 * are synthetic event streams (down/move/up + frame ticks) against a
 * fixed layout snapshot; effects are recorded by fakes. Coordinate
 * frame: 3 columns, cell 100x200, horizontal pitch 120, vertical pitch
 * 220, full-width headers 60 tall, viewport 340x2200.
 */
class AdjustDragControllerTest {
    private val g1 = PlaylistGroupId(1)
    private val g2 = PlaylistGroupId(2)

    private val snapshot: GridSnapshot = run {
        fun cell(baseTop: Float, slot: Int): RectF {
            val col = slot % 3
            val row = slot / 3
            val left = col * 120f
            val top = baseTop + row * 220f
            return RectF(left, top, left + 100f, top + 200f)
        }

        GridSnapshot(
            viewportTop = 0f,
            viewportBottom = 2200f,
            viewportWidth = 340f,
            sections = listOf(
                SectionGeometry(
                    groupId = g1,
                    header = RectF(0f, 0f, 340f, 60f),
                    expanded = true,
                    memberCount = 6,
                    visibleCards = (0..5).map {
                        CardGeometry(it, cell(76f, it), id = (it + 1).toLong())
                    },
                ),
                SectionGeometry(
                    groupId = g2,
                    header = RectF(0f, 600f, 340f, 660f),
                    expanded = false,
                    memberCount = 3,
                    visibleCards = emptyList(),
                ),
            ),
        )
    }

    private class Recorder {
        val scrolls = mutableListOf<Float>()
        val haptics = mutableListOf<Int>()
        val cardMoves = mutableListOf<Triple<PlaylistId, PlaylistGroupId, Pair<PlaylistGroupId, Int>>>()
        val groupMoves = mutableListOf<Pair<PlaylistGroupId, PlaylistGroupId?>>()
    }

    private fun controller(
        rec: Recorder,
        snapshotProvider: () -> GridSnapshot? = { null },
    ): AdjustDragController = AdjustDragController(
        scope = CoroutineScope(Dispatchers.Unconfined),
        scrollBy = { dy -> rec.scrolls.add(dy) },
        haptic = { rec.haptics.add(0) },
        commitCardMove = { id, from, to, slot ->
            rec.cardMoves.add(Triple(id, from, to to slot))
        },
        commitGroupMove = { id, before -> rec.groupMoves.add(id to before) },
        snapshotProvider = snapshotProvider,
        config = AdjustDragController.Config(
            touchSlop = 10f,
            longPressTimeoutMs = 500L,
        ),
    )

    // --- tap ---

    @Test
    fun `tap without claim records nothing`() {
        val rec = Recorder()
        val c = controller(rec)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onUp()
        assertEquals(AdjustPhase.Idle, c.state.phase)
        assertTrue(rec.scrolls.isEmpty())
        assertTrue(rec.haptics.isEmpty())
        assertTrue(rec.cardMoves.isEmpty())
    }

    // --- scroll / fling ---

    @Test
    fun `swipe past slop becomes scrolling with matching deltas`() {
        val rec = Recorder()
        val c = controller(rec)
        c.onDown(1, Vec2(50f, 300f), 0)
        // First move crosses slop — transitions only, no scroll yet.
        c.onMove(Vec2(50f, 350f), 16)
        assertEquals(AdjustPhase.Scrolling, c.state.phase)
        c.onMove(Vec2(50f, 352f), 116)
        c.onMove(Vec2(50f, 354f), 216)
        assertEquals(listOf(-2f, -2f), rec.scrolls)
        // Slow lift velocity (20 px/s over the last window): no fling.
        c.onUp()
        assertEquals(AdjustPhase.Idle, c.state.phase)
    }

    @Test
    fun `fast swipe release flings and decays to idle`() {
        val rec = Recorder()
        val c = controller(rec)
        c.onDown(1, Vec2(50f, 1000f), 0)
        c.onMove(Vec2(50f, 1100f), 16)
        c.onMove(Vec2(50f, 1200f), 32)
        c.onMove(Vec2(50f, 1280f), 48)
        c.onUp()
        assertEquals(AdjustPhase.Flinging, c.state.phase)
        // Prime the tick clock, then decay: dt = 0.1s.
        c.onTick(48_000_000L)
        c.onTick(148_000_000L)
        // v0 = (1280-1000)/0.048 ≈ 5833 px/s downward → scroll is negative.
        assertEquals(-583.33f, rec.scrolls.last(), 1f)
        // Burn 10s of frames: the fling must settle to Idle.
        var nowNs = 148_000_000L
        repeat(600) {
            nowNs += 16_666_667L
            c.onTick(nowNs)
        }
        assertEquals(AdjustPhase.Idle, c.state.phase)
    }

    @Test
    fun `drift before the long-press timeout scrolls instead of claiming`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onMove(Vec2(170f, 230f), 16) // drift 54 > slop 10
        assertEquals(AdjustPhase.Scrolling, c.state.phase)
        c.onTick(600_000_000L) // long-press deadline passes mid-scroll
        assertEquals(AdjustPhase.Scrolling, c.state.phase)
        c.onUp()
        assertTrue(rec.haptics.isEmpty())
        assertTrue(rec.cardMoves.isEmpty())
    }

    // --- long-press drag claim ---

    @Test
    fun `jitter within slop during the hold still claims the drag`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0)
        // Oscillating jitter: displacement never exceeds 4px, but the
        // traversed path is ~16px — must NOT become a scroll (this is
        // the round-10 residual: accumulated-path drift tripped slop).
        c.onMove(Vec2(174f, 176f), 100)
        c.onMove(Vec2(172f, 176f), 200)
        c.onMove(Vec2(174f, 176f), 300)
        c.onMove(Vec2(172f, 176f), 400)
        assertEquals(AdjustPhase.Pending, c.state.phase)
        c.onTick(0)
        c.onTick(600_000_000L)
        assertEquals(AdjustPhase.Dragging, c.state.phase)
        assertEquals(1, rec.haptics.size)
    }

    @Test
    fun `held drag survives a large first move after the claim`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L) // claim fires before any move
        assertEquals(AdjustPhase.Dragging, c.state.phase)
        // Large first delta (840px) post-claim: still a drag — a
        // claimed drag must never hand the gesture back to scrolling.
        c.onMove(Vec2(170f, 1016f), 640)
        assertEquals(AdjustPhase.Dragging, c.state.phase)
        assertTrue(rec.scrolls.isEmpty())
        c.onUp()
        assertEquals(1, rec.cardMoves.size)
    }

    @Test
    fun `long press on a card claims a drag with haptic`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        // Down on card slot 1's center, held still.
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L)
        assertEquals(AdjustPhase.Dragging, c.state.phase)
        assertEquals(1, rec.haptics.size)
        assertEquals(PlaylistId(2), c.state.draggedCardId)
        assertEquals(g1, c.state.originGroupId)
    }

    @Test
    fun `long press on empty space does not claim`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(50f, 505f), 0) // band gap below group A's rows
        c.onTick(0)
        c.onTick(600_000_000L)
        assertEquals(AdjustPhase.Pending, c.state.phase)
        c.onUp()
        assertEquals(AdjustPhase.Idle, c.state.phase)
        assertTrue(rec.haptics.isEmpty())
    }

    // --- commit ---

    @Test
    fun `drag lift commits the resolved target`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0) // card slot 1
        c.onTick(0)
        c.onTick(600_000_000L)
        // Move over card slot 0's left half, then lift.
        c.onMove(Vec2(30f, 176f), 620)
        c.onUp()
        assertEquals(1, rec.cardMoves.size)
        val (id, from, target) = rec.cardMoves.first()
        assertEquals(PlaylistId(2), id)
        assertEquals(g1, from)
        assertEquals(g1 to 0, target)
    }

    @Test
    fun `drag lift onto another group's header commits head of that group`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L)
        c.onMove(Vec2(50f, 620f), 620) // inside group B's (collapsed) header
        c.onUp()
        assertEquals(1, rec.cardMoves.size)
        val (id, from, target) = rec.cardMoves.first()
        assertEquals(PlaylistId(2), id)
        assertEquals(g1, from)
        assertEquals(g2 to 0, target)
    }

    @Test
    fun `cancel during drag resets without commit`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L)
        c.onMove(Vec2(30f, 176f), 620)
        c.onCancel()
        assertEquals(AdjustPhase.Idle, c.state.phase)
        c.onUp()
        assertTrue(rec.cardMoves.isEmpty())
    }

    // --- auto-scroll during drag ---

    @Test
    fun `drag near the bottom edge auto-scrolls forward`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L) // claim
        c.onTick(616_666_667L) // keep clock fresh (dt = 0.1s)
        c.onMove(Vec2(100f, 2100f), 640) // inside the 120px bottom band
        c.onTick(716_666_667L) // dt = exactly 0.1s
        // frac = (2100 - 2080)/120 → speed ≈ 300 + 1500*0.167 ≈ 550 px/s
        assertEquals(55f, rec.scrolls.last(), 5f)
        assertTrue(rec.scrolls.last() > 0f)
    }

    @Test
    fun `drag mid-viewport does not auto-scroll`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L)
        c.onTick(700_000_000L)
        c.onMove(Vec2(170f, 1000f), 620)
        val scrollsBefore = rec.scrolls.size
        c.onTick(800_000_000L)
        assertEquals(scrollsBefore, rec.scrolls.size)
    }

    // --- render: zone → indicator visual ---

    @Test
    fun `render draws the header bar for header and head zones`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L)
        // Over group B's header band.
        c.onMove(Vec2(50f, 620f), 640)
        val render = c.render()!!
        assertNull(render.cardIndicator)
        assertEquals(snapshot.sections[1].header!!.bottom, render.barY)
        // In the gap above group A's first row (head strip).
        c.onMove(Vec2(50f, 70f), 680)
        val headRender = c.render()!!
        assertNull(headRender.cardIndicator)
        assertEquals(snapshot.sections[0].header!!.bottom, headRender.barY)
    }

    @Test
    fun `render outlines the hovered card for precise positions`() {
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { snapshot })
        c.onLayout(snapshot)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L)
        // Upper half of card slot 1 — must outline THAT card (not the first).
        c.onMove(Vec2(140f, 100f), 640)
        val render = c.render()!!
        assertEquals(snapshot.sections[0].visibleCards.first { it.slot == 1 }.rect, render.cardIndicator)
        assertNull(render.barY)
    }

    @Test
    fun `render draws the wrapped placeholder cell for a full last row end`() {
        val onlyA = GridSnapshot(
            viewportTop = 0f,
            viewportBottom = 2200f,
            viewportWidth = 340f,
            sections = listOf(snapshot.sections[0]),
        )
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { onlyA })
        c.onLayout(onlyA)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L)
        // Below the last row's cells (bottom 496) → END, wrapped cell
        // (top 496 + 20px row spacing).
        c.onMove(Vec2(30f, 800f), 640)
        val render = c.render()!!
        assertEquals(RectF(0f, 516f, 100f, 716f), render.cardIndicator)
        assertNull(render.barY)
    }

    @Test
    fun `render draws the trailing placeholder cell for a partial last row end`() {
        val partial = GridSnapshot(
            viewportTop = 0f,
            viewportBottom = 2200f,
            viewportWidth = 340f,
            sections = listOf(
                SectionGeometry(
                    groupId = g1,
                    header = RectF(0f, 0f, 340f, 60f),
                    expanded = true,
                    memberCount = 4,
                    visibleCards = (0..3).map { slot ->
                        val col = slot % 3
                        val row = slot / 3
                        val left = col * 120f
                        val top = 76f + row * 220f
                        CardGeometry(slot, RectF(left, top, left + 100f, top + 200f), id = (slot + 1).toLong())
                    },
                ),
            ),
        )
        val rec = Recorder()
        val c = controller(rec, snapshotProvider = { partial })
        c.onLayout(partial)
        c.onDown(1, Vec2(170f, 176f), 0)
        c.onTick(0)
        c.onTick(600_000_000L)
        // Into the empty trailing cell of the partial last row.
        c.onMove(Vec2(300f, 396f), 640)
        val render = c.render()!!
        assertEquals(RectF(120f, 296f, 220f, 496f), render.cardIndicator)
    }
}
