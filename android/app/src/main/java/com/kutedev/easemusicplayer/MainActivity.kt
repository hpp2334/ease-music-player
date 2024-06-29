package com.kutedev.easemusicplayer

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.ui.theme.EaseMusicPlayerTheme
import com.kutedev.easemusicplayer.viewmodels.PlaylistsViewModel
import com.kutedev.easemusicplayer.widgets.playlists.PlaylistsPage
import uniffi.ease_client.ArgInitializeApp
import uniffi.ease_client.CreatePlaylistMode
import uniffi.ease_client.finishCreatePlaylist
import uniffi.ease_client.initializeClient
import uniffi.ease_client.prepareCreatePlaylist
import uniffi.ease_client.updateCreatePlaylistMode
import uniffi.ease_client.updateCreatePlaylistName

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        registerNotifies()

        val playlistsVM: PlaylistsViewModel by viewModels()

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
            EaseMusicPlayerTheme {
                Scaffold(modifier = Modifier.fillMaxSize()) { innerPadding ->
                    Box(modifier = Modifier.padding(innerPadding)) {
                        PlaylistsPage(playlistsViewModel = playlistsVM)
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

