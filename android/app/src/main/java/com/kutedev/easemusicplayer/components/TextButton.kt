package com.kutedev.easemusicplayer.components

import androidx.compose.material3.ButtonColors
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.sp

enum class EaseTextButtonType {
    Primary,
    Error,
    Default,
}

enum class EaseTextButtonSize {
    Medium,
    Small,
}

@Composable
fun EaseTextButton(
    text: String,
    type: EaseTextButtonType,
    size: EaseTextButtonSize,
    onClick: () -> Unit,
    disabled: Boolean = false,
) {
    val fontSize = when (size) {
        EaseTextButtonSize.Small -> 10.sp
        EaseTextButtonSize.Medium -> 14.sp
    }
    val buttonColors = when(type) {
        EaseTextButtonType.Default -> ButtonDefaults.textButtonColors()
        EaseTextButtonType.Primary -> {
            ButtonDefaults.textButtonColors().copy(
                contentColor = MaterialTheme.colorScheme.primary
            )
        }
        EaseTextButtonType.Error -> {
            ButtonDefaults.textButtonColors().copy(
                contentColor = MaterialTheme.colorScheme.error
            )
        }
    }

    TextButton(
        colors = buttonColors,
        onClick = onClick,
        enabled = !disabled
    ) {
        Text(
            text = text,
            fontSize = fontSize,
        )
    }
}