package com.kutedev.easemusicplayer.components

import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.unit.dp

enum class EaseIconButtonSize {
    Small,
    Medium,
    Large,
}

@Composable
fun EaseIconButton(
    sizeType: EaseIconButtonSize,
    painter: Painter,
    onClick: () -> Unit,
) {
    val buttonSize = run {
          when (sizeType) {
              EaseIconButtonSize.Small -> 24.dp
              EaseIconButtonSize.Medium -> 36.dp
              EaseIconButtonSize.Large -> 64.dp
          }
    }
    val buttonPadding = run {
        when (sizeType) {
            EaseIconButtonSize.Small -> 7.dp
            EaseIconButtonSize.Medium -> 10.dp
            EaseIconButtonSize.Large -> 20.dp
        }
    }

    IconButton(onClick = onClick, modifier = Modifier.width(buttonSize).height(buttonSize)) {
        Icon(
            painter = painter,
            contentDescription = null,
            modifier = Modifier.padding(buttonPadding)
        )
    }
}
