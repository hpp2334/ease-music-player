package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

class DesktopPermissionManager : PermissionManager {
    override val hasStoragePermission: StateFlow<Boolean> = MutableStateFlow(true)
    override fun requestStoragePermission() {}
}
