package com.kutedev.easemusicplayer

import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import com.kutedev.easemusicplayer.di.appModule
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import org.koin.core.context.GlobalContext.startKoin

fun main() = application {
    startKoin {
        modules(appModule)
    }

    val koin = org.koin.core.context.GlobalContext.get()
    val bridge = koin.get<Bridge>()
    bridge.initialize()

    val playerRepository = koin.get<PlayerRepository>()
    val storageRepository = koin.get<StorageRepository>()
    val playlistRepository = koin.get<PlaylistRepository>()
    CoroutineScope(SupervisorJob() + Dispatchers.Default).launch {
        playerRepository.reload()
        storageRepository.reload()
        playlistRepository.reload()
    }

    Window(
        onCloseRequest = ::exitApplication,
        title = "Ease Music Player",
        state = rememberWindowState(width = 800.dp, height = 600.dp),
    ) {
        Root()
    }
}
