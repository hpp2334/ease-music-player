package com.kutedev.easemusicplayer.widgets.playlists

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Pure-function tests for the drag-target geometry. Coordinate frame:
 * 3 columns, cell 100x200, horizontal pitch 120, vertical pitch 220
 * (20px gaps); full-width headers 60 tall.
 */
class DragGeometryTest {
    private val gridWidth = 340f // leftmost..rightmost card edge (0..340)

    private fun cellRect(baseTop: Float, slot: Int): RectF {
        val col = slot % 3
        val row = slot / 3
        val left = col * 120f
        val top = baseTop + row * 220f
        return RectF(left, top, left + 100f, top + 200f)
    }

    private fun section(
        id: String,
        memberCount: Int,
        visibleSlots: IntRange,
        headerTop: Float = 0f,
        expanded: Boolean = true,
        headerBottom: Float = headerTop + 60f,
    ) = SectionGeometry(
        groupId = id,
        header = RectF(0f, headerTop, gridWidth + 800f, headerBottom),
        expanded = expanded,
        memberCount = memberCount,
        visibleCards = visibleSlots.map { CardGeometry(it, cellRect(headerBottom + 16f, it)) },
    )

    // --- header drop zones ---

    @Test
    fun `drop on collapsed header targets group head`() {
        val a = section("A", 3, 0..2)
        val b = section("B", 2, IntRange.EMPTY, headerTop = 600f, expanded = false)
        val target = resolveCardDrop(listOf(a, b), Vec2(50f, 620f))!!
        assertEquals("B", target.groupId)
        assertEquals(0, target.slot)
        assertEquals(DropZone.Header, target.zone)
    }

    @Test
    fun `drop on expanded group's own header targets its head`() {
        val a = section("A", 6, 0..5)
        val target = resolveCardDrop(listOf(a), Vec2(400f, 30f))!!
        assertEquals("A", target.groupId)
        assertEquals(0, target.slot)
        assertEquals(DropZone.Header, target.zone)
    }

    @Test
    fun `gap above the first row is the head strip from any x`() {
        val a = section("A", 6, 0..5)
        // Between the header bottom (60) and the first row's top (76),
        // and above it — both resolve to the head.
        assertEquals(0 to DropZone.Head, resolveCardDrop(listOf(a), Vec2(300f, 68f))!!.let { it.slot to it.zone })
        assertEquals(0 to DropZone.Head, resolveCardDrop(listOf(a), Vec2(30f, -100f))!!.let { it.slot to it.zone })
    }

    @Test
    fun `upper half of a first-row card positions precisely`() {
        // Regression (user report): hovering the UPPER half of a first
        // row destination card used to resolve to the head strip and
        // outline the first card. It must position by card centers.
        val a = section("A", 6, 0..5)
        val target = resolveCardDrop(listOf(a), Vec2(140f, 100f))!!
        assertEquals(1, target.slot)
        assertEquals(DropZone.Position, target.zone)
    }

    @Test
    fun `drop below the last row's cells lands at section end from any x`() {
        val a = section("A", 6, 0..5)
        // Below row 1's bottom edge (496) — the whole strip is END.
        val target = resolveCardDrop(listOf(a), Vec2(30f, 560f))!!
        assertEquals("A", target.groupId)
        assertEquals(6, target.slot)
        assertEquals(DropZone.End, target.zone)
    }

    // --- within-group slot resolution ---

    @Test
    fun `drop between cards in first row lands before the crossed card`() {
        val a = section("A", 6, 0..5)
        val target = resolveCardDrop(listOf(a), Vec2(130f, 176f))!!
        assertEquals(1, target.slot)
        assertEquals(DropZone.Position, target.zone)
    }

    @Test
    fun `drop left of first card lands at slot zero`() {
        val a = section("A", 6, 0..5)
        val target = resolveCardDrop(listOf(a), Vec2(30f, 176f))!!
        assertEquals(0, target.slot)
        assertEquals(DropZone.Position, target.zone)
    }

    @Test
    fun `drop right of a full row lands at next row head`() {
        val a = section("A", 6, 0..5)
        val target = resolveCardDrop(listOf(a), Vec2(300f, 176f))!!
        assertEquals(3, target.slot)
    }

    @Test
    fun `drop over empty trailing cells of partial final row lands at section end`() {
        // 4 members: row 1 holds only slot 3 (+ two EMPTY cells).
        val a = section("A", 4, 0..3)
        val target = resolveCardDrop(listOf(a), Vec2(300f, 396f))!!
        assertEquals("A", target.groupId)
        assertEquals(4, target.slot)
        assertEquals(DropZone.Position, target.zone) // slot == memberCount → placeholder cell
    }

    @Test
    fun `drop below a group's cards but above next header lands at group end`() {
        val a = section("A", 6, 0..5)
        val b = section("B", 2, 0..1, headerTop = 600f, expanded = false)
        // Right of the last row's last card, in the gap above B's header.
        val target = resolveCardDrop(listOf(a, b), Vec2(300f, 560f))!!
        assertEquals("A", target.groupId)
        assertEquals(6, target.slot)
    }

    // --- band mapping / clamping ---

    @Test
    fun `pointer below first group's band maps into the next group`() {
        val a = section("A", 6, 0..5)
        val b = section("B", 2, 0..1, headerTop = 600f)
        // Inside B's first row (top 676), left of its second card's
        // center → before that card.
        val target = resolveCardDrop(listOf(a, b), Vec2(130f, 700f))!!
        assertEquals("B", target.groupId)
        assertEquals(1, target.slot)
        assertEquals(DropZone.Position, target.zone)
        // The thin gap between B's header bottom (660) and its row top
        // (676) is B's head strip.
        val head = resolveCardDrop(listOf(a, b), Vec2(130f, 668f))!!
        assertEquals("B", head.groupId)
        assertEquals(0, head.slot)
        assertEquals(DropZone.Head, head.zone)
    }

    @Test
    fun `pointer above first anchor clamps to the first group`() {
        val a = section("A", 6, 0..5)
        val target = resolveCardDrop(listOf(a), Vec2(30f, -100f))!!
        assertEquals("A", target.groupId)
        assertEquals(0, target.slot)
    }

    @Test
    fun `collapsed band without cards resolves to member count`() {
        val a = section("A", 6, 0..5)
        val b = section("B", 5, IntRange.EMPTY, headerTop = 600f, expanded = false)
        val target = resolveCardDrop(listOf(a, b), Vec2(50f, 700f))!!
        assertEquals("B", target.groupId)
        assertEquals(5, target.slot)
    }

    @Test
    fun `section without any visible geometry falls back to the first section`() {
        // Every anchor is NaN (nothing visible) — resolve to something sane.
        val a = section("A", 3, IntRange.EMPTY, expanded = false, header = null)
        val target = resolveCardDrop(listOf(a), Vec2(50f, 50f))!!
        assertEquals("A", target.groupId)
        assertEquals(3, target.slot)
    }

    private fun section(
        id: String,
        memberCount: Int,
        visibleSlots: IntRange,
        expanded: Boolean,
        header: RectF?,
    ) = SectionGeometry(id, header, expanded, memberCount, emptyList())

    @Test
    fun `empty sections list resolves to null`() {
        assertNull(resolveCardDrop(emptyList<SectionGeometry<String>>(), Vec2(10f, 10f)))
    }

    // --- indicator geometry ---

    @Test
    fun `indicator at an occupied slot is that card's rect`() {
        val a = section("A", 6, 0..5)
        val rect = cardSlotIndicatorRect(a, 4, gridWidth)!!
        assertEquals(cellRect(76f, 4), rect)
    }

    @Test
    fun `indicator past the row end continues the row when content allows`() {
        val a = section("A", 6, 0..5)
        val rect = cardSlotIndicatorRect(a, 6, 1080f)!!
        // Next cell right of slot 5 (row 1, col 2).
        assertEquals(RectF(360f, 296f, 460f, 496f), rect)
    }

    @Test
    fun `indicator wraps to a new row when the row is full`() {
        val a = section("A", 6, 0..5)
        val rect = cardSlotIndicatorRect(a, 6, gridWidth)!!
        // Wraps under row 1 with the 20px row spacing.
        assertEquals(RectF(0f, 516f, 100f, 716f), rect)
    }

    @Test
    fun `indicator is null for a section without visible cards`() {
        val a = section("A", 2, IntRange.EMPTY)
        assertNull(cardSlotIndicatorRect(a, 0, gridWidth))
    }

    @Test
    fun `end indicator of a partial row lands in the empty trailing cell`() {
        // 4 members: row 1 = [slot3, EMPTY, EMPTY] — the placeholder
        // must sit in the real empty column (exact pitch from row 0).
        val a = section("A", 4, 0..3)
        val rect = cardSlotIndicatorRect(a, 4, 1080f)!!
        assertEquals(RectF(120f, 296f, 220f, 496f), rect)
    }

    // --- group (header) reorder ---

    @Test
    fun `group insertion index counts headers above the pointer`() {
        val rects = listOf(
            RectF(0f, 0f, 100f, 60f),      // center 30
            RectF(0f, 100f, 100f, 160f),   // center 130
            RectF(0f, 200f, 100f, 260f),   // center 230
        )
        assertEquals(0, insertionIndexByCenterY(rects, 10f))
        assertEquals(1, insertionIndexByCenterY(rects, 50f))
        assertEquals(2, insertionIndexByCenterY(rects, 150f))
        assertEquals(3, insertionIndexByCenterY(rects, 300f))
    }

    @Test
    fun `group insertion bar sits above the target header or below the last`() {
        val rects = listOf(
            RectF(0f, 0f, 100f, 60f),
            RectF(0f, 100f, 100f, 160f),
        )
        assertEquals(-2f, insertionBarY(rects, 0))
        assertEquals(98f, insertionBarY(rects, 1))
        assertEquals(162f, insertionBarY(rects, 2))
        assertNull(insertionBarY(emptyList(), 0))
    }
}
