package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

class DesktopPermissionChecker : PermissionChecker {
    override val havePermission: StateFlow<Boolean> = MutableStateFlow(true)
    override fun requestStoragePermission() {}
}
