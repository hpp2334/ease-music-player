package com.kutedev.easemusicplayer

import com.kutedev.easemusicplayer.di.appModule
import com.kutedev.easemusicplayer.di.desktopModule
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.DesktopPlayerController
import com.kutedev.easemusicplayer.singleton.PlayerController
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import javafx.application.Platform
import org.koin.core.context.startKoin

fun main() {
    val nativeLibDir = "../rust-libs/target/debug"
    System.setProperty("jna.library.path", nativeLibDir)

    val koin = startKoin {
        modules(appModule, desktopModule)
    }.koin

    val bridge = koin.get<Bridge>()
    bridge.initialize()

    val playerController = koin.get<PlayerController>() as DesktopPlayerController

    application {
        Platform.startup { }

        Window(
            onCloseRequest = {
                playerController.destroy()
                bridge.destroy()
                Platform.exit()
                exitApplication()
            },
            title = "Ease Music Player"
        ) {
            Root()
        }
    }
}
