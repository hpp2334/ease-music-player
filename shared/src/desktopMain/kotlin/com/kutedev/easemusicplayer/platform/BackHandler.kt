package com.kutedev.easemusicplayer.platform

import androidx.compose.runtime.Composable

@Composable
actual fun BackHandler(enabled: Boolean, onBack: () -> Unit) {
    // Desktop: no system back button, typically handled by keyboard shortcuts
}
