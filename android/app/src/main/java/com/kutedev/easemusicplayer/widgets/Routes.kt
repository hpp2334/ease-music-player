package com.kutedev.easemusicplayer.widgets

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController

enum class RoutesKey(val value: String) {
    HOME("HOME"),
    ADD_DEVICES("ADD_DEVICES"),
    PLAYLIST("PLAYLIST"),
    IMPORT_MUSICS("IMPORT_MUSICS"),
    MUSIC_PLAYER("MUSIC_PLAYER")
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
