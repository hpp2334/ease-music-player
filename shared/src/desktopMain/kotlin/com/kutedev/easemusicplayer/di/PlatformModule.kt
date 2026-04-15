package com.kutedev.easemusicplayer.di

import com.kutedev.easemusicplayer.singleton.DesktopPermissionChecker
import com.kutedev.easemusicplayer.singleton.DesktopPlayerController
import com.kutedev.easemusicplayer.singleton.PermissionChecker
import com.kutedev.easemusicplayer.singleton.PlayerController
import org.koin.core.module.Module
import org.koin.dsl.module

actual val platformModule: Module = module {
    single<PlayerController> { DesktopPlayerController() }
    single<PermissionChecker> { DesktopPermissionChecker() }
}
