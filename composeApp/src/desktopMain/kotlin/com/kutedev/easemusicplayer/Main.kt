package com.kutedev.easemusicplayer

import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import com.kutedev.easemusicplayer.di.appModule
import com.kutedev.easemusicplayer.di.desktopModule
import com.kutedev.easemusicplayer.singleton.Bridge
import org.koin.core.context.startKoin

fun main() {
    val nativeLibDir = "../rust-libs/target/debug"
    System.setProperty("jna.library.path", nativeLibDir)

    val koin = startKoin {
        modules(appModule, desktopModule)
    }.koin

    val bridge = koin.get<Bridge>()
    bridge.initialize()

    application {
        Window(
            onCloseRequest = {
                bridge.destroy()
                exitApplication()
            },
            title = "Ease Music Player"
        ) {
            App()
        }
    }
}
