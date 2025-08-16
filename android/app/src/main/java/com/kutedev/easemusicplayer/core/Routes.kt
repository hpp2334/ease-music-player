package com.kutedev.easemusicplayer.core

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController
import kotlinx.serialization.Serializable

@Serializable
object RouteHome

@Serializable
object RouteAddDevices

@Serializable
data class RoutePlaylist(
    val id: Long
)

enum class RouteImportType {
    Music,
    Lyric,
    EditPlaylist,
    EditPlaylistCover
}

@Serializable
data class RouteImport(
    val type: RouteImportType,
    val id: Long,
)

@Serializable
object RouteMusicPlayer

val LocalNavController = compositionLocalOf<NavHostController> {
    error("No LocalNavController provided")
}

inline fun <reified T: Any> NavBackStackEntry.matches(): Boolean {
    return T::class.qualifiedName == destination.route
}

@Composable
fun RoutesProvider(
    block: @Composable () -> Unit
) {
    CompositionLocalProvider(LocalNavController provides rememberNavController()) {
        block()
    }
}
