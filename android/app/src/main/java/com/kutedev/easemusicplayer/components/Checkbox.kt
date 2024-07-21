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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.R

@Composable
fun EaseCheckbox(
    value: Boolean,
    onChange: (value: Boolean) -> Unit
) {
    val borderColor = if (value) { MaterialTheme.colorScheme.primary } else { MaterialTheme.colorScheme.onSurface }
    val bgColor = if (value) { Color.Transparent } else { MaterialTheme.colorScheme.primary }

    Box(
        modifier = Modifier
            .size(32.dp)
            .background(bgColor)
            .border(1.dp, borderColor, RoundedCornerShape(2.dp))
            .clickable { onChange(!value) },
        contentAlignment = Alignment.Center
    ) {
        if (value) {
            Icon(
                painter = painterResource(id = R.drawable.icon_yes),
                tint = MaterialTheme.colorScheme.surface,
                contentDescription = null,
                modifier = Modifier.width(6.dp)
            )
        }
    }
}