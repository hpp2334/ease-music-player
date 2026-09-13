package com.kutedev.easemusicplayer.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.R

@Composable
fun EaseCheckbox(
    value: Boolean,
    onChange: (value: Boolean) -> Unit,
    disabled: Boolean = false
) {
    val activeColor = if (value) {
        MaterialTheme.colorScheme.primary
    } else {
        Color.Transparent
    }
    val dim = if (disabled) {
        0.38F
    } else {
        1F
    }
    val borderColor = if (value) {
        MaterialTheme.colorScheme.primary.copy(alpha = dim)
    } else {
        MaterialTheme.colorScheme.onSurface.copy(alpha = dim)
    }

    Box(
        modifier = Modifier
            .border(1.dp, borderColor, RoundedCornerShape(4.dp))
            .clip(RoundedCornerShape(4.dp))
            .size(16.dp)
            .background(activeColor.copy(alpha = dim))
            .then(
                if (disabled) {
                    Modifier
                } else {
                    Modifier.clickable { onChange(!value) }
                }
            ),
        contentAlignment = Alignment.Center
    ) {
        if (value) {
            Icon(
                painter = painterResource(id = R.drawable.icon_yes),
                tint = MaterialTheme.colorScheme.surface.copy(alpha = dim),
                contentDescription = null,
                modifier = Modifier.width(6.dp)
            )
        }
    }
}
