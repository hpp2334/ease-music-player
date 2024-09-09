package com.kutedev.easemusicplayer

import androidx.compose.runtime.compositionLocalOf
import androidx.navigation.NavHostController

object Routes {
    const val HOME = "Home";
    const val ADD_DEVICES = "AddDevices";
    const val PLAYLIST = "Playlist";
    const val IMPORT_MUSICS = "ImportMusics";
    const val MUSIC_PLAYER = "MusicPlayer";
}

val LocalNavController = compositionLocalOf<NavHostController> {
    error("No LocalNavController provided")
}