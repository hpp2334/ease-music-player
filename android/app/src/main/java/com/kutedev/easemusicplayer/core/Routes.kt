package com.kutedev.easemusicplayer.core

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController

fun RouteHome(): String {
    return "Home"
}

fun isRouteHome(route: String): Boolean {
    return route == "Home"
}

fun RouteCreateStorage(): String {
    return "CreateStorage"
}

fun RouteEditStorage(id: String): String {
    return "EditStorage/${id}"
}

fun isRouteEditStorage(route: String): Boolean {
    return route.startsWith("EditStorage/")
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

fun RouteLog(): String {
    return "Debug/Log"
}

fun RouteDebugMore(): String {
    return "Debug/More"
}

fun RoutePluginManagement(): String {
    return "PluginManagement"
}

fun isRoutePluginManagement(route: String): Boolean {
    return route == "PluginManagement"
}

fun RoutePluginAvailable(): String {
    return "PluginAvailable"
}

fun isRoutePluginAvailable(route: String): Boolean {
    return route == "PluginAvailable"
}

fun RoutePluginView(pluginId: String, viewId: String): String {
    return "PluginView/${pluginId}/${viewId}"
}

fun isRoutePluginView(route: String): Boolean {
    return route.startsWith("PluginView/")
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
