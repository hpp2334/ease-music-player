package com.kutedev.easemusicplayer.di

import com.kutedev.easemusicplayer.singleton.PermissionChecker
import com.kutedev.easemusicplayer.singleton.PermissionRepository
import com.kutedev.easemusicplayer.singleton.PlayerController
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import org.koin.core.module.Module
import org.koin.core.module.dsl.singleOf
import org.koin.dsl.bind
import org.koin.dsl.module

actual val platformModule: Module = module {
    single { PlayerControllerRepository(get(), get(), get(), get(), get(), get()) } bind PlayerController::class
    single { PermissionRepository(get()) } bind PermissionChecker::class
}
