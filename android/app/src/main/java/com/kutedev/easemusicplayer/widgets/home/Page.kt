package com.kutedev.easemusicplayer.widgets.home

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.viewmodel.compose.viewModel
import com.kutedev.easemusicplayer.viewmodels.PlayerVM
import com.kutedev.easemusicplayer.widgets.appbar.BottomBar
import com.kutedev.easemusicplayer.widgets.appbar.getBottomBarSpace
import com.kutedev.easemusicplayer.widgets.dashboard.DashboardSubpage
import com.kutedev.easemusicplayer.widgets.playlists.PlaylistsSubpage
import com.kutedev.easemusicplayer.widgets.settings.SettingSubpage

@Composable
fun HomePage(
    playerVM: PlayerVM = viewModel(),
    scaffoldPadding: PaddingValues,
) {
    val pagerState = rememberPagerState(pageCount = {
        3
    })
    val musicState by playerVM.musicState.collectAsState()
    val isPlaying = musicState.playing

    Box(
        modifier = Modifier.fillMaxSize(),
    ) {
        HorizontalPager(
            modifier = Modifier.padding(
                bottom = getBottomBarSpace(isPlaying, scaffoldPadding),
            ),
            state = pagerState
        ) { page ->
            if (page == 0) {
                PlaylistsSubpage()
            }
            if (page == 1) {
                DashboardSubpage()
            }
            if (page == 2) {
                SettingSubpage()
            }
        }
        BottomBar(
            bottomBarPageState = pagerState,
            scaffoldPadding = scaffoldPadding,
        )
    }
}