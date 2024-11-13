package com.kutedev.easemusicplayer.widgets.home

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import com.kutedev.easemusicplayer.viewmodels.EaseViewModel
import com.kutedev.easemusicplayer.widgets.appbar.BottomBar
import com.kutedev.easemusicplayer.widgets.appbar.getBottomBarSpace
import com.kutedev.easemusicplayer.widgets.dashboard.DashboardSubpage
import com.kutedev.easemusicplayer.widgets.playlists.PlaylistsSubpage
import com.kutedev.easemusicplayer.widgets.settings.SettingSubpage
import uniffi.ease_client.RoutesKey

@Composable
fun HomePage(
    ctx: android.content.Context,
    pagerState: PagerState,
    evm: EaseViewModel,
    scaffoldPadding: PaddingValues,
) {
    val state by evm.currentMusicState.collectAsState()
    val isPlaying = state.playing

    Box(
        modifier = Modifier.fillMaxSize(),
    ) {
        HorizontalPager(
            modifier = Modifier.padding(
                bottom = getBottomBarSpace(isPlaying),
            ),
            state = pagerState
        ) { page ->
            if (page == 0) {
                PlaylistsSubpage(
                    evm = evm,
                )
            }
            if (page == 1) {
                DashboardSubpage(evm = evm)
            }
            if (page == 2) {
                SettingSubpage(ctx = ctx)
            }
        }
        BottomBar(
            currentRoute = RoutesKey.HOME,
            bottomBarPageState = pagerState,
            evm = evm,
            scaffoldPadding = scaffoldPadding,
        )
    }
}