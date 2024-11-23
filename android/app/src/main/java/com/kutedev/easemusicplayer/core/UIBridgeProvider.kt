package com.kutedev.easemusicplayer.core

import androidx.compose.runtime.compositionLocalOf


val UIBridgeController = compositionLocalOf<UIBridge> {
    error("No UIBridgeController provided")
}
