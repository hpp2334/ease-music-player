package com.kutedev.easemusicplayer.components

import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

fun Modifier.dropShadow(
    color: Color,
    offsetX: Dp,
    offsetY: Dp,
    blurRadius: Dp,
) = then(
    drawBehind {
        val leftPixel = offsetX.toPx()
        val topPixel = offsetY.toPx()
        val rightPixel = size.width + topPixel
        val bottomPixel = size.height + leftPixel

        drawRect(
            color = color,
            topLeft = androidx.compose.ui.geometry.Offset(leftPixel, topPixel),
            size = androidx.compose.ui.geometry.Size(
                rightPixel - leftPixel,
                bottomPixel - topPixel
            )
        )
    }
)
