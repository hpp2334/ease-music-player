package com.kutedev.easemusicplayer

import com.kutedev.easemusicplayer.di.appModule
import com.kutedev.easemusicplayer.di.desktopModule
import com.kutedev.easemusicplayer.platform.TrayController
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.DesktopPlayerController
import com.kutedev.easemusicplayer.singleton.PlayerController
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import javafx.application.Platform
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
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
    val playerRepository = koin.get<PlayerRepository>()

    application {
        Platform.startup { }

        val windowState = rememberWindowState()
        var windowVisible by remember { mutableStateOf(true) }
        val trayScope = remember { CoroutineScope(SupervisorJob()) }

        var doQuit: (() -> Unit)? = null

        val tray = remember {
            TrayController(
                playingFlow = playerRepository.playing,
                scope = trayScope,
                onShow = { windowVisible = true },
                onPlayPauseToggle = {
                    if (playerRepository.playing.value) {
                        playerController.pause()
                    } else {
                        playerController.resume()
                    }
                },
                onQuit = { doQuit?.invoke() }
            )
        }

        doQuit = {
            playerController.destroy()
            bridge.destroy()
            tray.remove()
            trayScope.cancel()
            Platform.exit()
            exitApplication()
        }

        DisposableEffect(Unit) {
            tray.install()
            onDispose {
                tray.remove()
                trayScope.cancel()
            }
        }

        Window(
            onCloseRequest = {
                if (tray.isInstalled) {
                    windowVisible = false
                } else {
                    doQuit?.invoke()
                }
            },
            visible = windowVisible,
            state = windowState,
            title = "Ease Music Player"
        ) {
            Root()
        }
    }
}
