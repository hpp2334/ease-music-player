package com.kutedev.easemusicplayer.widgets.playlists

import kotlin.math.abs
import kotlin.math.sqrt

/**
 * Pure drag-target geometry for the playlists page.
 *
 * No Compose / Android dependencies — the Compose layer (see
 * [PlaylistDragController]) maps `LazyGridState.layoutInfo` into these
 * plain-float structures and calls the resolution functions here, both
 * for the live drop indicator and for the single commit on release.
 *
 * The core principle: drop targets are derived from POINTER GEOMETRY
 * against real layout bounds (headers, card centers, row structure),
 * never from item indices in a flat list — grid row ends, empty
 * trailing cells and group boundaries therefore resolve unambiguously.
 */

data class Vec2(val x: Float, val y: Float) {
    operator fun plus(o: Vec2): Vec2 = Vec2(x + o.x, y + o.y)
    operator fun minus(o: Vec2): Vec2 = Vec2(x - o.x, y - o.y)
    fun getDistance(): Float = sqrt(x * x + y * y)

    companion object {
        val Zero = Vec2(0f, 0f)
    }
}

/** Why a card drop target was chosen — drives the indicator visual. */
enum class DropZone {
    /** Pointer on a group's header band. */
    Header,
    /** Pointer in the gap between a section anchor and its first row. */
    Head,
    /** Positioned among the cards (row/column resolution). */
    Position,
    /** Past the group's last row (or a collapsed group's band): END. */
    End,
}

data class RectF(
    val left: Float,
    val top: Float,
    val right: Float,
    val bottom: Float,
) {
    val width: Float get() = right - left
    val height: Float get() = bottom - top
    val centerX: Float get() = (left + right) / 2f
    val centerY: Float get() = (top + bottom) / 2f

    fun contains(p: Vec2): Boolean =
        p.x >= left && p.x < right && p.y >= top && p.y < bottom
}

/**
 * One visible playlist card: its slot index inside its group plus a
 * caller-defined identity (the playlist id value — rides along so the
 * controller can address dragged items without knowing app types).
 */
class CardGeometry(val slot: Int, val rect: RectF, val id: Long = 0L)

/**
 * One group's visible geometry: its header bounds (null when scrolled
 * out), expand state, total member count (authoritative — some members
 * may be scrolled out of view) and its visible cards.
 */
class SectionGeometry<T>(
    val groupId: T,
    val header: RectF?,
    val expanded: Boolean,
    val memberCount: Int,
    val visibleCards: List<CardGeometry>,
)

/** Resolved drop target for a card drag: destination group + insert slot + zone. */
data class CardDropTarget<T>(val groupId: T, val slot: Int, val zone: DropZone)

/**
 * Resolve a card drop from the pointer position.
 *
 * Rules (in order):
 * 1. A pointer inside ANY group header (collapsed or expanded) drops
 *    at the HEAD of that group — "the header is the top of the group".
 * 2. Otherwise the pointer falls into exactly one section's vertical
 *    band (from the section's own anchor — header bottom, else first
 *    visible card top — down to the next section's anchor; the last
 *    band extends to +∞). Clamped to the first/last band when outside.
 * 3. Within an expanded section, full-width finger-friendly strips:
 *    below the LAST row's centerline = END of the group; above the
 *    FIRST row's centerline = HEAD (clamped to the first visible slot
 *    when the top is scrolled out). Between centerlines the slot is
 *    resolved by card centers, row-major: before the first card whose
 *    center follows the pointer; right of a row's last card means the
 *    next row's head — so empty trailing cells in a partial final row
 *    are unambiguous.
 *
 * Collapsed sections / sections without visible cards resolve to their
 * member count (end) when hit below the header.
 */
fun <T> resolveCardDrop(
    sections: List<SectionGeometry<T>>,
    pointer: Vec2,
): CardDropTarget<T>? {
    if (sections.isEmpty()) {
        return null
    }

    // 1) Header drop zones → head of that group.
    for (section in sections) {
        val header = section.header ?: continue
        if (header.contains(pointer)) {
            return CardDropTarget(section.groupId, 0, DropZone.Header)
        }
    }

    // 2) Vertical bands per section.
    val anchors = sections.map { section ->
        when {
            section.header != null -> section.header.bottom
            section.visibleCards.isNotEmpty() -> section.visibleCards.minOf { it.rect.top }
            else -> Float.NaN
        }
    }
    var chosen = -1
    for (i in sections.indices) {
        if (anchors[i].isNaN()) {
            continue
        }
        var bottom = Float.POSITIVE_INFINITY
        for (j in i + 1 until sections.size) {
            if (!anchors[j].isNaN()) {
                bottom = anchors[j]
                break
            }
        }
        if (pointer.y >= anchors[i] && pointer.y < bottom) {
            chosen = i
            break
        }
    }
    if (chosen < 0) {
        val first = anchors.indexOfFirst { !it.isNaN() }
        val last = anchors.indexOfLast { !it.isNaN() }
        chosen = when {
            first >= 0 && pointer.y < anchors[first] -> first
            last >= 0 -> last
            else -> return CardDropTarget(sections.first().groupId, sections.first().memberCount, DropZone.End)
        }
    }

    val section = sections[chosen]
    if (!section.expanded) {
        return CardDropTarget(section.groupId, section.memberCount, DropZone.End)
    }
    val (slot, zone) = resolveSlotInGroup(section, pointer)
    return CardDropTarget(section.groupId, slot, zone)
}

/** Slot resolution within one expanded section (see [resolveCardDrop] rule 3). */
private fun <T> resolveSlotInGroup(
    section: SectionGeometry<T>,
    pointer: Vec2,
): Pair<Int, DropZone> {
    val cards = section.visibleCards.sortedWith(compareBy({ it.rect.top }, { it.rect.left }))
    if (cards.isEmpty()) {
        return section.memberCount to DropZone.End
    }

    // Group into rows: cards sharing the same top (uniform grid rows).
    val rows = ArrayList<List<CardGeometry>>()
    var row = ArrayList<CardGeometry>()
    var rowTop = cards.first().rect.top
    for (card in cards) {
        if (abs(card.rect.top - rowTop) > 1f) {
            rows.add(row)
            row = ArrayList()
            rowTop = card.rect.top
        }
        row.add(card)
    }
    rows.add(row)

    // Row-EDGE strips (not centerlines — the upper/lower halves of the
    // edge rows must keep their precise card-center resolution):
    // strictly above the first row's cells = HEAD; strictly below the
    // last row's cells = END.
    val firstTop = rows.first().first().rect.top
    val lastBottom = rows.last().first().rect.bottom
    if (pointer.y < firstTop) {
        return rows.first().minOf { it.slot } to DropZone.Head
    }
    if (pointer.y > lastBottom) {
        return section.memberCount to DropZone.End
    }

    // Nearest row by vertical center (pointer between rows picks the closer).
    val rowIndex = rows.indices.minByOrNull { index ->
        abs(pointer.y - rows[index].first().rect.centerY)
    } ?: 0

    // Before the first card in the row whose center follows the pointer.
    for (card in rows[rowIndex].sortedBy { it.rect.left }) {
        if (pointer.x < card.rect.centerX) {
            return card.slot to DropZone.Position
        }
    }
    // Right of the row's last card: the next row's head, or the section
    // end for the last row (a group's last row may have empty trailing
    // cells — those drops resolve to the end).
    if (rowIndex == rows.lastIndex) {
        return section.memberCount to DropZone.Position
    }
    return rows[rowIndex + 1].minOf { it.slot } to DropZone.Position
}

/**
 * Bounds of the insertion indicator for [slot] inside [section]:
 * the crossed card's rect, or the next row-major cell past the last
 * visible card (approximate for scrolled-out members — visual only;
 * the committed slot always comes from [resolveCardDrop]).
 * `contentRight` is the right edge of the grid content area.
 * Returns null when the section shows no cards (the caller draws a
 * bar under the header instead).
 */
fun cardSlotIndicatorRect(
    section: SectionGeometry<*>,
    slot: Int,
    contentRight: Float,
): RectF? {
    val cards = section.visibleCards.sortedBy { it.slot }
    if (cards.isEmpty()) {
        return null
    }
    cards.firstOrNull { it.slot >= slot }?.let { return it.rect }

    // Past the last visible card → the next row-major cell: the empty
    // trailing cell when the last row is partial, else the wrapped cell
    // below. The pitch comes from the section's actual full rows (not a
    // guess), so the placeholder lands in the real empty column.
    val last = cards.last()
    val horizontalPitch = globalHorizontalPitch(cards)
        ?: (last.rect.width + 16f)
    val nextLeft = last.rect.left + horizontalPitch
    if (nextLeft + last.rect.width <= contentRight + 0.5f) {
        return RectF(nextLeft, last.rect.top, nextLeft + last.rect.width, last.rect.bottom)
    }
    // Wrap to a new row below.
    val minLeft = cards.minOf { it.rect.left }
    val tops = cards.map { it.rect.top }.distinct().sorted()
    val rowGap = tops.zipWithNext().minOfOrNull { (a, b) -> b - a } ?: (last.rect.height + 16f)
    val spacing = (rowGap - last.rect.height).coerceAtLeast(8f)
    val top = last.rect.bottom + spacing
    return RectF(minLeft, top, minLeft + last.rect.width, top + last.rect.height)
}

/** Column pitch derived from any full row of the section (min across rows). */
private fun globalHorizontalPitch(cards: List<CardGeometry>): Float? {
    val byRow = cards.groupBy { it.rect.top }
    return byRow.values
        .mapNotNull { row ->
            val sorted = row.sortedBy { it.rect.left }
            if (sorted.size >= 2) {
                sorted[1].rect.left - sorted[0].rect.left
            } else {
                null
            }
        }
        .minOrNull()
}

/**
 * Insertion index among group headers (group reorder drag): the number
 * of headers (in group order) whose center is above the pointer.
 * [headerRects] must be the visible headers EXCLUDING the dragged one.
 */
fun insertionIndexByCenterY(headerRects: List<RectF>, pointerY: Float): Int {
    var index = 0
    for (rect in headerRects) {
        if (pointerY > rect.centerY) {
            index++
        } else {
            break
        }
    }
    return index
}

/**
 * Vertical position for the group-insertion indicator bar: just above
 * the header at [insertionIndex], or just below the last header when
 * inserting at the end.
 */
fun insertionBarY(headerRects: List<RectF>, insertionIndex: Int): Float? {
    if (headerRects.isEmpty()) {
        return null
    }
    return if (insertionIndex < headerRects.size) {
        headerRects[insertionIndex].top - 2f
    } else {
        headerRects.last().bottom + 2f
    }
}
