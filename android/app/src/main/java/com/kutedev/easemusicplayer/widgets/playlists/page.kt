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
import androidx.activity.viewModels
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.IconButton
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.TextUnitType
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import uniffi.ease_client.VPlaylistListState

@Composable
fun PlaylistsPage(playlistsViewModel: PlaylistsViewModel) {
    val state = playlistsViewModel.state.collectAsState().value

    Column {
        Row {
           EaseIconButton(
               sizeType = EaseIconButtonSize.Medium,
               painter = painterResource(id = R.drawable.icon_plus),
               onClick = { /*TODO*/ }
           )
        }
        GridPlaylists(playlists = state.playlistList)
    }
}

@Composable
private fun GridPlaylists(playlists: List<VPlaylistAbstractItem>) {
    LazyVerticalGrid(columns = GridCells.FixedSize(150.dp)) {
        items(playlists) { playlist ->
            PlaylistItem(playlist = playlist)
        }
    }
}

@Composable
private fun PlaylistItem(playlist: VPlaylistAbstractItem) {
    Column(
        modifier = Modifier.padding(8.dp),
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