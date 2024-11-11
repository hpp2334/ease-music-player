package com.kutedev.easemusicplayer

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.PagerState
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.core.IOnNotifyView
import com.kutedev.easemusicplayer.ui.theme.EaseMusicPlayerTheme
import com.kutedev.easemusicplayer.utils.nextTickOnMain
import com.kutedev.easemusicplayer.viewmodels.CreatePlaylistViewModel
import com.kutedev.easemusicplayer.viewmodels.CurrentMusicViewModel
import com.kutedev.easemusicplayer.viewmodels.CurrentPlaylistViewModel
import com.kutedev.easemusicplayer.viewmodels.CurrentStorageEntriesViewModel
import com.kutedev.easemusicplayer.viewmodels.EditPlaylistViewModel
import com.kutedev.easemusicplayer.viewmodels.PlaylistsViewModel
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel
import com.kutedev.easemusicplayer.widgets.appbar.BottomBar
import com.kutedev.easemusicplayer.viewmodels.EditStorageFormViewModel
import com.kutedev.easemusicplayer.widgets.RoutesProvider
import com.kutedev.easemusicplayer.widgets.dashboard.TimeToPauseModal
import com.kutedev.easemusicplayer.widgets.devices.EditStoragesPage
import com.kutedev.easemusicplayer.widgets.home.HomePage
import com.kutedev.easemusicplayer.widgets.musics.ImportMusicsPage
import com.kutedev.easemusicplayer.widgets.musics.MusicPlayerPage
import com.kutedev.easemusicplayer.widgets.playlists.PlaylistPage
import com.kutedev.easemusicplayer.widgets.playlists.CreatePlaylistsDialog
import com.kutedev.easemusicplayer.widgets.playlists.EditPlaylistsDialog
import uniffi.ease_client.MainAction
import uniffi.ease_client.RoutesKey
import uniffi.ease_client.ViewAction

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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        registerViewModels()

        val requestPermissionLauncher = registerForActivityResult(ActivityResultContracts.RequestPermission()) { isGranted ->
            nextTickOnMain {
                Bridge.dispatchAction(ViewAction.Main(MainAction.PERMISSION_CHANGED))
            }
        }
        Bridge.onActivityCreate(applicationContext, requestPermissionLauncher)

        setContent {
            Root()
        }
    }

    override fun onStart() {
        super.onStart()
        Bridge.onActivityStart()
    }

    override fun onStop() {
        super.onStop()
        Bridge.onActivityStop()
    }

    override fun onDestroy() {
        super.onDestroy()
        Bridge.onActivityDestroy()

        for (destroy in vmDestroyers) {
            destroy()
        }
        vmDestroyers.clear()
    }

    @Composable
    fun Root() {
        val playlistsVM: PlaylistsViewModel by viewModels()
        val timeToPauseVM: TimeToPauseViewModel by viewModels()
        val storageListVM: StorageListViewModel by viewModels()
        val editStorageVM: EditStorageFormViewModel by viewModels()
        val createPlaylistVM: CreatePlaylistViewModel by viewModels()
        val currentPlaylistVM: CurrentPlaylistViewModel by viewModels()
        val currentStorageEntriesVM: CurrentStorageEntriesViewModel by viewModels()
        val currentMusicVM: CurrentMusicViewModel by viewModels()
        val editPlaylistVM: EditPlaylistViewModel by viewModels()

        RoutesProvider { controller ->
            EaseMusicPlayerTheme {
                Scaffold(
                    modifier = Modifier.fillMaxSize(),
                ) { innerPadding ->
                    Column(
                        modifier = Modifier
                            .padding(innerPadding)
                            .fillMaxSize()
                    ) {
                        val bottomBarPageState = rememberPagerState(pageCount = {
                            3
                        })

                        Box(
                            modifier = Modifier.weight(1f)
                        ) {
                            NavHost(
                                modifier = Modifier
                                    .fillMaxSize(),
                                navController = controller,
                                startDestination = RoutesKey.HOME.toString(),
                                enterTransition = { EnterTransition.None },
                                exitTransition = { ExitTransition.None },
                                popEnterTransition = { EnterTransition.None },
                                popExitTransition = { ExitTransition.None },
                            ) {
                                composable(RoutesKey.HOME.toString()) {
                                    HomePage(
                                        ctx = applicationContext,
                                        pagerState = bottomBarPageState,
                                        currentMusicVM = currentMusicVM,
                                        playlistsVM = playlistsVM,
                                        timeToPauseVM = timeToPauseVM,
                                        storageListVM = storageListVM,
                                    )
                                    CreatePlaylistsDialog(vm = createPlaylistVM)
                                }
                                composable(RoutesKey.ADD_DEVICES.toString()) {
                                    EditStoragesPage(
                                        formVM = editStorageVM,
                                    )
                                }
                                composable(RoutesKey.PLAYLIST.toString()) {
                                    PlaylistPage(
                                        vm = currentPlaylistVM,
                                        currentMusicVM = currentMusicVM,
                                    )
                                    EditPlaylistsDialog(vm = editPlaylistVM)
                                }
                                composable(RoutesKey.IMPORT_MUSICS.toString()) {
                                    ImportMusicsPage(vm = currentStorageEntriesVM)
                                }
                                composable(RoutesKey.MUSIC_PLAYER.toString()) {
                                    MusicPlayerPage(
                                        vm = currentMusicVM,
                                        timeToPauseVM = timeToPauseVM,
                                    )
                                }
                            }
                            TimeToPauseModal(vm = timeToPauseVM)
                        }
                    }
                }
            }
        }
    }

    private fun registerViewModels() {
        registerViewModel<PlaylistsViewModel>()
        registerViewModel<StorageListViewModel>()
        registerViewModel<EditStorageFormViewModel>()
        registerViewModel<CreatePlaylistViewModel>()
        registerViewModel<EditPlaylistViewModel>()
        registerViewModel<CurrentPlaylistViewModel>()
        registerViewModel<CurrentStorageEntriesViewModel>()
        registerViewModel<CurrentMusicViewModel>()
        registerViewModel<TimeToPauseViewModel>()
    }
}

