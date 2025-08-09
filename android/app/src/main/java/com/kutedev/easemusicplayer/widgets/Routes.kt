package com.kutedev.easemusicplayer.widgets

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController
import kotlinx.serialization.Serializable
import uniffi.ease_client_backend.MusicId

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
    Lyric
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

@Composable
fun RoutesProvider(
    block: @Composable () -> Unit
) {
    CompositionLocalProvider(LocalNavController provides rememberNavController()) {
        block()
    }
}
