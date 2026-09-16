package com.kutedev.easemusicplayer.core

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.navigation.NavHostController
import androidx.navigation.NavOptionsBuilder
import androidx.navigation.compose.rememberNavController
import androidx.navigation.NavController

/**
 * Navigate to [route] with `launchSingleTop` — a double-tap (or any double
 * fire) of a card must NOT stack a second copy of the destination. Two
 * live copies of a page hosting a [com.kutedev.easemusicplayer.turintegration.TurView]
 * race their surface attaches and wgpu aborts the whole process on
 * `ERROR_NATIVE_WINDOW_IN_USE_KHR` (the 2026-09-16/17 play-counts crash).
 */
fun NavController.navigateSingleTop(route: String, builder: (NavOptionsBuilder.() -> Unit)? = null) {
    navigate(route) {
        launchSingleTop = true
        builder?.invoke(this)
    }
}

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

fun RouteLyricParser(): String {
    return "LyricParser"
}

fun isRouteLyricParser(route: String): Boolean {
    return route == "LyricParser"
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
