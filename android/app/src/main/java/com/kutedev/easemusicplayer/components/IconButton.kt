package com.kutedev.easemusicplayer.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonColors
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.res.colorResource
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.R

enum class EaseIconButtonSize {
    Small,
    Medium,
    Large,
}

enum class EaseIconButtonType {
    Default,
    Surface,
    Primary,
    Error,
}

data class EaseIconButtonColors(
    val buttonBg: Color,
    val iconTint: Color,
)

@Composable
fun EaseIconButton(
    sizeType: EaseIconButtonSize,
    buttonType: EaseIconButtonType,
    painter: Painter,
    onClick: () -> Unit,
    overrideColors: EaseIconButtonColors? = null,
    disabled: Boolean = false,
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
    val buttonBg = run {
        overrideColors?.buttonBg
            ?: when (buttonType) {
                EaseIconButtonType.Primary -> MaterialTheme.colorScheme.primary
                EaseIconButtonType.Surface -> Color.Transparent
                EaseIconButtonType.Default -> Color.Transparent
                EaseIconButtonType.Error -> Color.Transparent
            }
    }
    val iconTint = run {
        overrideColors?.iconTint
            ?: when (buttonType) {
            EaseIconButtonType.Primary -> Color.White
            EaseIconButtonType.Surface -> MaterialTheme.colorScheme.surface
            EaseIconButtonType.Default -> MaterialTheme.colorScheme.onSurface
            EaseIconButtonType.Error -> MaterialTheme.colorScheme.error
        }
    }

    IconButton(
        onClick = onClick,
        modifier = Modifier
            .width(buttonSize)
            .height(buttonSize),
        enabled = !disabled,
        colors = IconButtonColors(
            buttonBg,
            iconTint,
            Color.Transparent,
            MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Icon(
            painter = painter,
            contentDescription = null,
            modifier = Modifier.padding(buttonPadding),
        )
    }
}
