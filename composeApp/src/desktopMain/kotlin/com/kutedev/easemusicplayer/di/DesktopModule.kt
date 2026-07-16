package com.kutedev.easemusicplayer.di

import com.kutedev.easemusicplayer.platform.AppPaths
import com.kutedev.easemusicplayer.singleton.DesktopPermissionManager
import com.kutedev.easemusicplayer.singleton.DesktopPlayerController
import com.kutedev.easemusicplayer.singleton.PermissionManager
import com.kutedev.easemusicplayer.singleton.PlayerController
import org.koin.dsl.module

val desktopModule = module {
    single {
        AppPaths(
            documentDir = System.getProperty("user.home") + "/.ease-music-player/",
            cacheDir = System.getProperty("java.io.tmpdir") + "/ease-music-player/",
        )
    }
    single<PlayerController> {
        DesktopPlayerController(get(), get(), get(), get(), get(), get())
    }
    single<PermissionManager> { DesktopPermissionManager() }
}
