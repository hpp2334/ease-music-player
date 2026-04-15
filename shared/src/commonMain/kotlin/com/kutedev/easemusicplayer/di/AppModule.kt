package com.kutedev.easemusicplayer.di

import com.kutedev.easemusicplayer.singleton.AssetRepository
import com.kutedev.easemusicplayer.platform.getAppCacheDir
import com.kutedev.easemusicplayer.platform.getAppDocumentDir
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
import org.koin.core.module.dsl.viewModel
import org.koin.dsl.module

val appModule = module {
    includes(platformModule)

    single { CoroutineScope(SupervisorJob() + Dispatchers.Default) }
    single { Bridge(getAppDocumentDir(), getAppCacheDir(), get()) }
    single { PlayerRepository(get(), get()) }
    single { StorageRepository(get(), get()) }
    single { ToastRepository(get()) }
    single { ImportRepository() }
    single { AssetRepository(get()) }
    single { PlaylistRepository(get(), get(), get()) }

    viewModel { PlayerVM(get(), get()) }
    viewModel { PlaylistVM(get(), get(), get(), get(), get(), get()) }
    viewModel { PlaylistsVM(get()) }
    viewModel { AssetVM(get()) }
    viewModel { ImportVM(get(), get(), get(), get()) }
    viewModel { CreatePlaylistVM(get(), get()) }
    viewModel { EditPlaylistVM(get(), get(), get()) }
    viewModel { EditStorageVM(get(), get(), get(), get()) }
    viewModel { SleepModeVM(get()) }
    viewModel { StoragesVM(get()) }
    viewModel { ToastVM(get()) }
    viewModel { LogVM(get()) }
    viewModel { DebugMoreVM(get()) }
}
