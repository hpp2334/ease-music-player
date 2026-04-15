package com.kutedev.easemusicplayer.components

import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp

expect fun Modifier.dropShadow(
    color: Color,
    offsetX: Dp,
    offsetY: Dp,
    blurRadius: Dp,
): Modifier
