package com.kutedev.easemusicplayer.utils

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

private val mainScope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

fun nextTickOnMain(f: () -> Unit) {
    mainScope.launch { f() }
}
