package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.flow.StateFlow

interface PermissionChecker {
    val havePermission: StateFlow<Boolean>
    fun requestStoragePermission()
}
