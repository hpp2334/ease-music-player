package com.kutedev.easemusicplayer.core

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController
import com.kutedev.easemusicplayer.repositories.RouteImportType
import kotlinx.serialization.Serializable

fun RouteHome(): String {
    return "Home"
}

fun isRouteHome(route: String): Boolean {
    return route == "Home"
}

fun RouteAddDevices(id: String): String {
    return "AddDevices/${id}"
}

fun RoutePlaylist(id: String): String {
    return "Playlist/${id}"
}

fun isRoutePlaylist(route: String): Boolean {
    return route.startsWith("Playlist/")
}

fun RouteImport(type: String): String {
    return "Import/${type}"
}

fun RouteMusicPlayer(): String {
    return "MusicPlayer"
}

val LocalNavController = compositionLocalOf<NavHostController> {
    error("No LocalNavController provided")
}

@Composable
fun RoutesProvider(
    block: @Composable () -> Unit
) {
    CompositionLocalProvider(LocalNavController provides rememberNavController()) {
        block()
    }
}
