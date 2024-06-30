package com.kutedev.easemusicplayer

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.Scaffold
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.unit.dp
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.ui.theme.EaseMusicPlayerTheme
import com.kutedev.easemusicplayer.viewmodels.PlaylistsViewModel
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel
import com.kutedev.easemusicplayer.widgets.appbar.BottomBar
import com.kutedev.easemusicplayer.widgets.home.HomePage
import uniffi.ease_client.CreatePlaylistMode
import uniffi.ease_client.finishCreatePlaylist
import uniffi.ease_client.prepareCreatePlaylist
import uniffi.ease_client.updateCreatePlaylistMode
import uniffi.ease_client.updateCreatePlaylistName

class MainActivity : ComponentActivity() {
    @OptIn(ExperimentalFoundationApi::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        registerNotifies()

        val playlistsVM: PlaylistsViewModel by viewModels()
        val timeToPauseVM: TimeToPauseViewModel by viewModels()
        val storageListVM: StorageListViewModel by viewModels()

        Bridge.initApp(applicationContext)

        Bridge.invoke { prepareCreatePlaylist() }
        Bridge.invoke { updateCreatePlaylistMode(CreatePlaylistMode.EMPTY) }
        Bridge.invoke { updateCreatePlaylistName("ABC") }
        Bridge.invoke { finishCreatePlaylist() }

        Bridge.invoke { prepareCreatePlaylist() }
        Bridge.invoke { updateCreatePlaylistMode(CreatePlaylistMode.EMPTY) }
        Bridge.invoke { updateCreatePlaylistName("容器") }
        Bridge.invoke { finishCreatePlaylist() }

        Bridge.invoke { prepareCreatePlaylist() }
        Bridge.invoke { updateCreatePlaylistMode(CreatePlaylistMode.EMPTY) }
        Bridge.invoke { updateCreatePlaylistName("GBC!!!") }
        Bridge.invoke { finishCreatePlaylist() }

        setContent {
            val bottomBarPageState = rememberPagerState(pageCount = {
                3
            })

            EaseMusicPlayerTheme {
                Scaffold(
                    modifier = Modifier.fillMaxSize(),
                ) { innerPadding ->
                    Box(
                        modifier = Modifier
                            .padding(innerPadding)
                            .fillMaxSize()
                    ) {
                        HomePage(
                            pagerState = bottomBarPageState,
                            playlistsVM = playlistsVM,
                            timeToPauseVM = timeToPauseVM,
                            storageListVM = storageListVM,
                        )

                        Box(
                            modifier = Modifier
                                .align(Alignment.BottomStart)
                                .height(60.dp)
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

    override fun onDestroy() {
        super.onDestroy()

        run {
            val vm: PlaylistsViewModel by viewModels();
            Bridge.unregisterView(vm);
        }
    }

    private fun registerNotifies() {
        run {
            val vm: PlaylistsViewModel by viewModels();
            Bridge.registerView(vm);
        }
    }
}

