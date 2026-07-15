package com.kutedev.easemusicplayer.di

import com.kutedev.easemusicplayer.platform.AppPaths
import com.kutedev.easemusicplayer.platform.appContext
import com.kutedev.easemusicplayer.singleton.PermissionManager
import com.kutedev.easemusicplayer.singleton.PermissionRepository
import com.kutedev.easemusicplayer.singleton.PlayerController
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import org.koin.dsl.module

val androidModule = module {
    single {
        AppPaths(
            documentDir = appContext.filesDir.absolutePath,
            cacheDir = appContext.cacheDir.absolutePath,
        )
    }
    single<PlayerController> { PlayerControllerRepository(get(), get(), get(), get(), get(), get()) }
    single<PermissionManager> { PermissionRepository(get()) }
}
