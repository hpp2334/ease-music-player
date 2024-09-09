package com.kutedev.easemusicplayer

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.PagerState
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.core.IOnNotifyView
import com.kutedev.easemusicplayer.ui.theme.EaseMusicPlayerTheme
import com.kutedev.easemusicplayer.viewmodels.CreatePlaylistViewModel
import com.kutedev.easemusicplayer.viewmodels.CurrentMusicViewModel
import com.kutedev.easemusicplayer.viewmodels.CurrentPlaylistViewModel
import com.kutedev.easemusicplayer.viewmodels.CurrentStorageEntriesViewModel
import com.kutedev.easemusicplayer.viewmodels.PlaylistsViewModel
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel
import com.kutedev.easemusicplayer.widgets.appbar.BottomBar
import com.kutedev.easemusicplayer.viewmodels.EditStorageFormViewModel
import com.kutedev.easemusicplayer.widgets.devices.EditStoragesPage
import com.kutedev.easemusicplayer.widgets.home.HomePage
import com.kutedev.easemusicplayer.widgets.musics.ImportMusicsPage
import com.kutedev.easemusicplayer.widgets.musics.MusicPlayerPage
import com.kutedev.easemusicplayer.widgets.playlists.PlaylistPage

inline fun <reified T> MainActivity.registerViewModel()
where T : ViewModel, T : IOnNotifyView {
    val vm: T by viewModels()
    Bridge.registerView(vm)

    vmDestroyers.add {
        Bridge.unregisterView(vm)
    }
}

class MainActivity : ComponentActivity() {
    val vmDestroyers = mutableListOf<() -> Unit>()

    @OptIn(ExperimentalFoundationApi::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        registerNotifies()

        Bridge.initApp(applicationContext)

        setContent {
            val bottomBarPageState = rememberPagerState(pageCount = {
                3
            })

            EaseMusicPlayerTheme {
                Scaffold(
                    modifier = Modifier.fillMaxSize(),
                ) { innerPadding ->
                    CompositionLocalProvider(LocalNavController provides rememberNavController()) {
                        Column(
                            modifier = Modifier
                                .padding(innerPadding)
                                .fillMaxSize()
                        ) {
                            Box(
                                modifier = Modifier.weight(1f)
                            ) {
                                RouteBlock(
                                    bottomBarPageState = bottomBarPageState,
                                )
                            }
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .shadow(2.dp)
                            ) {
                                BottomBar(bottomBarPageState)
                            }
                        }
                    }
                }
            }
        }
    }

    @OptIn(ExperimentalFoundationApi::class)
    @Composable
    fun RouteBlock(
        bottomBarPageState: PagerState,
    ) {
        val playlistsVM: PlaylistsViewModel by viewModels()
        val timeToPauseVM: TimeToPauseViewModel by viewModels()
        val storageListVM: StorageListViewModel by viewModels()
        val editStorageVM: EditStorageFormViewModel by viewModels()
        val createPlaylistVM: CreatePlaylistViewModel by viewModels()
        val currentPlaylistVM: CurrentPlaylistViewModel by viewModels()
        val currentStorageEntriesVM: CurrentStorageEntriesViewModel by viewModels()
        val currentMusicVM: CurrentMusicViewModel by viewModels()

        NavHost(
            navController = LocalNavController.current,
            startDestination = Routes.HOME
        ) {
            composable(Routes.HOME) {
                HomePage(
                    ctx = applicationContext,
                    pagerState = bottomBarPageState,
                    playlistsVM = playlistsVM,
                    createPlaylistVM = createPlaylistVM,
                    timeToPauseVM = timeToPauseVM,
                    storageListVM = storageListVM,
                )
            }
            composable(Routes.ADD_DEVICES) {
                EditStoragesPage(
                    formVM = editStorageVM,
                )
            }
            composable(Routes.PLAYLIST) {
                PlaylistPage(
                    vm = currentPlaylistVM,
                    currentMusicVM = currentMusicVM,
                )
            }
            composable(Routes.IMPORT_MUSICS) {
                ImportMusicsPage(currentStorageEntriesVM = currentStorageEntriesVM)
            }
            composable(Routes.MUSIC_PLAYER) {
                MusicPlayerPage(vm = currentMusicVM)
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()

        for (destroy in vmDestroyers) {
            destroy()
        }
        vmDestroyers.clear()
    }

    private fun registerNotifies() {
        registerViewModel<PlaylistsViewModel>()
        registerViewModel<StorageListViewModel>()
        registerViewModel<EditStorageFormViewModel>()
        registerViewModel<CreatePlaylistViewModel>()
        registerViewModel<CurrentPlaylistViewModel>()
        registerViewModel<CurrentStorageEntriesViewModel>()
        registerViewModel<CurrentMusicViewModel>()
    }
}

