package com.kutedev.easemusicplayer

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.core.IOnNotifyView
import com.kutedev.easemusicplayer.ui.theme.EaseMusicPlayerTheme
import com.kutedev.easemusicplayer.viewmodels.EditStorageViewModel
import com.kutedev.easemusicplayer.viewmodels.PlaylistsViewModel
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel
import com.kutedev.easemusicplayer.widgets.appbar.BottomBar
import com.kutedev.easemusicplayer.widgets.devices.EditStoragesPage
import com.kutedev.easemusicplayer.widgets.home.HomePage
import uniffi.ease_client.ArgUpsertStorage
import uniffi.ease_client.CreatePlaylistMode
import uniffi.ease_client.StorageType
import uniffi.ease_client.finishCreatePlaylist
import uniffi.ease_client.prepareCreatePlaylist
import uniffi.ease_client.updateCreatePlaylistMode
import uniffi.ease_client.updateCreatePlaylistName
import uniffi.ease_client.upsertStorage

class MainActivity : ComponentActivity() {
    @OptIn(ExperimentalFoundationApi::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        registerNotifies()

        val playlistsVM: PlaylistsViewModel by viewModels()
        val timeToPauseVM: TimeToPauseViewModel by viewModels()
        val storageListVM: StorageListViewModel by viewModels()
        val editStorageVM: EditStorageViewModel by viewModels()

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
                                NavHost(
                                    navController = LocalNavController.current,
                                    startDestination = Routes.Home
                                ) {
                                    composable(Routes.Home) {
                                        HomePage(
                                            ctx = applicationContext,
                                            pagerState = bottomBarPageState,
                                            playlistsVM = playlistsVM,
                                            timeToPauseVM = timeToPauseVM,
                                            storageListVM = storageListVM,
                                        )
                                    }
                                    composable(Routes.AddDevices) {
                                        EditStoragesPage(editStorageVM = editStorageVM)
                                    }
                                }
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

    override fun onDestroy() {
        super.onDestroy()

        run {
            val vm: PlaylistsViewModel by viewModels()
            Bridge.unregisterView(vm)
        }
        run {
            val vm: StorageListViewModel by viewModels()
            Bridge.unregisterView(vm)
        }
        run {
            val vm: EditStorageViewModel by viewModels()
            Bridge.unregisterView(vm)
        }
    }

    private fun registerNotifies() {
        run {
            val vm: PlaylistsViewModel by viewModels()
            Bridge.registerView(vm)
        }
        run {
            val vm: StorageListViewModel by viewModels()
            Bridge.registerView(vm)
        }
        run {
            val vm: EditStorageViewModel by viewModels()
            Bridge.registerView(vm)
        }
    }
}

