package com.kutedev.easemusicplayer.ui.theme

import android.app.Activity
import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalContext
import com.kutedev.easemusicplayer.turintegration.EaseThemesHost

private val DarkColorScheme = darkColorScheme(
    primary = Color(0xFF2E89B0),
    secondary = Color(0xFFC9EBFA),
    tertiary = Pink80,
    surfaceVariant = Color(0xFF303030)
)

private val LightColorScheme = lightColorScheme(
    primary = Color(0xFF2E89B0),
    secondary = Color(0xFFC9EBFA),
    tertiary = Pink40,
    surfaceVariant = Color(0xFFE3E3E3)

    /* Other default colors to override
    background = Color(0xFFFFFBFE),
    surface = Color(0xFFFFFBFE),
    onPrimary = Color.White,
    onSecondary = Color.White,
    onTertiary = Color.White,
    onBackground = Color(0xFF1C1B1F),
    */
)

@Composable
fun EaseMusicPlayerTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    // Dynamic color is available on Android 12+
    dynamicColor: Boolean = true,
    content: @Composable () -> Unit
) {
    val colorScheme = when {
//        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
//            val context = LocalContext.current
//            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
//        }
        darkTheme -> DarkColorScheme
        else -> LightColorScheme
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        content = content
    )

    // Push the resolved theme into `EaseThemesHost` so plugin JS (tur views)
    // can read it via the `ease:themes` host module. Re-synced on every
    // recomposition (dark/light toggle, etc.).
    SideEffect {
        EaseThemesHost.setTheme(
            isDark = darkTheme,
            colors = buildMap {
                put("primary", colorScheme.primary.hex())
                put("onPrimary", colorScheme.onPrimary.hex())
                put("primaryContainer", colorScheme.primaryContainer.hex())
                put("onPrimaryContainer", colorScheme.onPrimaryContainer.hex())
                put("secondary", colorScheme.secondary.hex())
                put("onSecondary", colorScheme.onSecondary.hex())
                put("secondaryContainer", colorScheme.secondaryContainer.hex())
                put("onSecondaryContainer", colorScheme.onSecondaryContainer.hex())
                put("tertiary", colorScheme.tertiary.hex())
                put("onTertiary", colorScheme.onTertiary.hex())
                put("background", colorScheme.background.hex())
                put("onBackground", colorScheme.onBackground.hex())
                put("surface", colorScheme.surface.hex())
                put("onSurface", colorScheme.onSurface.hex())
                put("surfaceVariant", colorScheme.surfaceVariant.hex())
                put("onSurfaceVariant", colorScheme.onSurfaceVariant.hex())
                put("surfaceContainer", colorScheme.surfaceContainer.hex())
                put("outline", colorScheme.outline.hex())
                put("outlineVariant", colorScheme.outlineVariant.hex())
                put("error", colorScheme.error.hex())
                put("onError", colorScheme.onError.hex())
            },
        )
    }
}

/** Format a Compose [Color] as a `"#RRGGBBAA"` hex string (tur's Color.hex
 *  expects RGBA, not Android's ARGB). All Material3 scheme colors are opaque. */
private fun Color.hex(): String {
    val argb = toArgb()
    val a = (argb ushr 24) and 0xFF
    val r = (argb ushr 16) and 0xFF
    val g = (argb ushr 8) and 0xFF
    val b = argb and 0xFF
    return "#%02X%02X%02X%02X".format(r, g, b, a)
}
