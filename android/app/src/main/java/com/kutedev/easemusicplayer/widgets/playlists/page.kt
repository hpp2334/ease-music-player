package com.kutedev.easemusicplayer.widgets.playlists

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.viewmodels.PlaylistsViewModel
import uniffi.ease_client.VPlaylistAbstractItem
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType

@Composable
fun PlaylistsSubpage(playlistsVM: PlaylistsViewModel) {
    val state = playlistsVM.state.collectAsState().value

    Column(
        modifier = Modifier.fillMaxSize()
    ) {
        Row(
            modifier = Modifier
                .padding(24.dp, 8.dp)
                .fillMaxWidth(),
            horizontalArrangement = Arrangement.End
        ) {
           EaseIconButton(
               sizeType = EaseIconButtonSize.Medium,
               buttonType = EaseIconButtonType.Default,
               painter = painterResource(id = R.drawable.icon_plus),
               onClick = { /*TODO*/ }
           )
        }
        GridPlaylists(playlists = state.playlistList)
    }
}

@Composable
private fun GridPlaylists(playlists: List<VPlaylistAbstractItem>) {
    LazyVerticalGrid(
        columns = GridCells.FixedSize(168.dp),
        horizontalArrangement = Arrangement.Center,
    ) {
        items(playlists) { playlist ->
            PlaylistItem(playlist = playlist)
        }
    }
}

@Composable
private fun PlaylistItem(playlist: VPlaylistAbstractItem) {
    Box(Modifier
        .clickable(
            onClick = {},
        )
    ) {
        Column(
            modifier = Modifier.padding(24.dp, 8.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.Start
        ) {
            Image(
                painter = painterResource(id = R.drawable.cover_default_image),
                contentDescription = null,
                modifier = Modifier.clip(RoundedCornerShape(20.dp))
            )
            Text(
                text = playlist.title,
                fontSize = 16.sp,
                modifier = Modifier.padding(top = 8.dp)
            )
            Text(
                text = buildAnnotatedString {
                    append("${playlist.count} ${stringResource(id = R.string.playlist_count_unit)}")
                    append("  ·  ")
                    append(playlist.duration)
                },
                fontSize = 12.sp,
                fontWeight = FontWeight.Light,
            )
        }
    }
}