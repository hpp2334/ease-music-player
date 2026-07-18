package com.kutedev.easemusicplayer.di

import com.kutedev.easemusicplayer.lifecycle.AppLifecycle
import com.kutedev.easemusicplayer.singleton.AssetRepository
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.ImportRepository
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import com.kutedev.easemusicplayer.singleton.ToastRepository
import com.kutedev.easemusicplayer.viewmodels.AssetVM
import com.kutedev.easemusicplayer.viewmodels.CreatePlaylistVM
import com.kutedev.easemusicplayer.viewmodels.DebugMoreVM
import com.kutedev.easemusicplayer.viewmodels.EditPlaylistVM
import com.kutedev.easemusicplayer.viewmodels.EditStorageVM
import com.kutedev.easemusicplayer.viewmodels.ImportVM
import com.kutedev.easemusicplayer.viewmodels.LogVM
import com.kutedev.easemusicplayer.viewmodels.PlayerVM
import com.kutedev.easemusicplayer.viewmodels.PlaylistVM
import com.kutedev.easemusicplayer.viewmodels.PlaylistsVM
import com.kutedev.easemusicplayer.viewmodels.SleepModeVM
import com.kutedev.easemusicplayer.viewmodels.StoragesVM
import com.kutedev.easemusicplayer.viewmodels.ToastVM
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.koin.core.module.dsl.singleOf
import org.koin.core.module.dsl.viewModelOf
import org.koin.dsl.module

val appModule = module {
    single<CoroutineScope> { CoroutineScope(SupervisorJob() + Dispatchers.Default) }
    singleOf(::ToastRepository)
    singleOf(::Bridge)
    singleOf(::AssetRepository)
    singleOf(::ImportRepository)
    singleOf(::PlayerRepository)
    singleOf(::StorageRepository)
    singleOf(::PlaylistRepository)
    singleOf(::AppLifecycle)

    viewModelOf(::PlaylistsVM)
    viewModelOf(::SleepModeVM)
    viewModelOf(::LogVM)
    viewModelOf(::StoragesVM)
    viewModelOf(::PlayerVM)
    viewModelOf(::EditPlaylistVM)
    viewModelOf(::AssetVM)
    viewModelOf(::PlaylistVM)
    viewModelOf(::CreatePlaylistVM)
    viewModelOf(::ImportVM)
    viewModelOf(::EditStorageVM)
    viewModelOf(::DebugMoreVM)
    viewModelOf(::ToastVM)
}
