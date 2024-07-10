package com.kutedev.easemusicplayer

import androidx.compose.runtime.compositionLocalOf
import androidx.navigation.NavHostController
import kotlinx.serialization.Serializable

object Routes {
    val Home = "Home";
    val AddDevices = "AddDevices";
}

val LocalNavController = compositionLocalOf<NavHostController> {
    error("No LocalNavController provided")
}