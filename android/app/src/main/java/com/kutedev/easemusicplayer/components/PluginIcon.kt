package com.kutedev.easemusicplayer.components

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.R

/**
 * Rounded icon tile for plugin-originated art: base64 [iconData] decoded
 * into a bitmap, or the built-in extension glyph when absent/undecodable.
 * The tile's fill and glyph tint both derive from [tint]. Shared by the
 * dashboard cards, the installed-plugins list, and the registry
 * (Get Plugins) list.
 */
@Composable
fun PluginIconBox(
    iconData: String?,
    tint: Color,
    modifier: Modifier = Modifier,
    boxSize: Dp = 44.dp,
    glyphSize: Dp = 24.dp,
    loading: Boolean = false,
) {
    Box(
        modifier = modifier
            .size(boxSize)
            .clip(RoundedCornerShape(12.dp))
            .background(tint.copy(alpha = 0.15f)),
        contentAlignment = Alignment.Center,
    ) {
        when {
            loading -> CircularProgressIndicator(
                modifier = Modifier.size(glyphSize * 0.9f),
                strokeWidth = 2.dp,
            )
            else -> {
                val icon = rememberPluginIcon(iconData)
                if (icon != null) {
                    // Fixed viewport (same footprint as the fallback glyph)
                    // so a plugin icon can't out-shout the app's own icon
                    // language whatever resolution it ships. `Fit`
                    // letterboxes non-square art safely inside it.
                    Image(
                        bitmap = icon.asImageBitmap(),
                        contentDescription = null,
                        contentScale = ContentScale.Fit,
                        modifier = Modifier.size(glyphSize),
                    )
                } else {
                    Icon(
                        modifier = Modifier.size(glyphSize),
                        painter = painterResource(id = R.drawable.icon_extension),
                        contentDescription = null,
                        tint = tint,
                    )
                }
            }
        }
    }
}

/**
 * Decode a plugin icon (`iconData` base64 from the Rust scan or registry
 * fetch) into a bitmap, memoized per payload. `null` → caller shows the
 * built-in glyph (missing icon, failed validation, or undecodable bytes).
 */
@Composable
fun rememberPluginIcon(iconData: String?): Bitmap? {
    if (iconData == null) {
        return null
    }
    return remember(iconData) {
        try {
            val bytes = Base64.decode(iconData, Base64.DEFAULT)
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
        } catch (_: IllegalArgumentException) {
            null
        }
    }
}
