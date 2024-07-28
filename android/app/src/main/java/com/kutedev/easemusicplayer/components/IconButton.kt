package com.kutedev.easemusicplayer.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.res.colorResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.R

enum class EaseIconButtonSize {
    Small,
    Medium,
    Large,
}

fun easeIconButtonSizeToDp(sizeType: EaseIconButtonSize): Dp {
    return when (sizeType) {
        EaseIconButtonSize.Small -> 24.dp
        EaseIconButtonSize.Medium -> 36.dp
        EaseIconButtonSize.Large -> 64.dp
    }
}

enum class EaseIconButtonType {
    Default,
    Surface,
    Primary,
    Error,
    ErrorVariant,
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
    val buttonSize = easeIconButtonSizeToDp(sizeType)
    val isVariant = buttonType == EaseIconButtonType.Primary || buttonType == EaseIconButtonType.ErrorVariant
    val iconSize = run {
        when (sizeType) {
            EaseIconButtonSize.Small -> 10.dp
            EaseIconButtonSize.Medium -> 16.dp
            EaseIconButtonSize.Large -> 24.dp
        }
    }
    val buttonBg = run {
        if (disabled) {
            if (!isVariant) {
                Color.Transparent
            } else {
                MaterialTheme.colorScheme.surfaceVariant
            }
        } else {
            overrideColors?.buttonBg
                ?: when (buttonType) {
                    EaseIconButtonType.Primary -> MaterialTheme.colorScheme.primary
                    EaseIconButtonType.Surface -> Color.Transparent
                    EaseIconButtonType.Default -> Color.Transparent
                    EaseIconButtonType.Error -> Color.Transparent
                    EaseIconButtonType.ErrorVariant -> MaterialTheme.colorScheme.error
                }
        }
    }
    val iconTint = run {
        if (disabled) {
            if (!isVariant) {
                MaterialTheme.colorScheme.onSurfaceVariant
            } else {
                MaterialTheme.colorScheme.surface
            }
        } else {
            overrideColors?.iconTint
                ?: when (buttonType) {
                    EaseIconButtonType.Primary -> MaterialTheme.colorScheme.surface
                    EaseIconButtonType.Surface -> MaterialTheme.colorScheme.surface
                    EaseIconButtonType.Default -> MaterialTheme.colorScheme.onSurface
                    EaseIconButtonType.Error -> MaterialTheme.colorScheme.error
                    EaseIconButtonType.ErrorVariant -> MaterialTheme.colorScheme.surface
                }
        }
    }

    Box(
        modifier = Modifier
            .size(buttonSize)
            .clip(RoundedCornerShape(999.dp))
            .background(buttonBg)
            .clickable(
                enabled = !disabled,
                onClick = {
                    onClick()
                }
            ),
        contentAlignment = Alignment.Center
    ) {
        Icon(
            painter = painter,
            contentDescription = null,
            modifier = Modifier.size(iconSize),
            tint = iconTint,
        )
    }
}
