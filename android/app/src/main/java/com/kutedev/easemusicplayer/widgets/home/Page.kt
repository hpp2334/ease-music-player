package com.kutedev.easemusicplayer.widgets.home

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import com.kutedev.easemusicplayer.viewmodels.CreatePlaylistViewModel
import com.kutedev.easemusicplayer.viewmodels.CurrentMusicViewModel
import com.kutedev.easemusicplayer.viewmodels.PlaylistsViewModel
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel
import com.kutedev.easemusicplayer.widgets.appbar.BottomBar
import com.kutedev.easemusicplayer.widgets.dashboard.DashboardSubpage
import com.kutedev.easemusicplayer.widgets.playlists.PlaylistsSubpage
import com.kutedev.easemusicplayer.widgets.settings.SettingSubpage
import uniffi.ease_client.RoutesKey

@Composable
fun HomePage(
    ctx: android.content.Context,
    pagerState: PagerState,
    currentMusicVM: CurrentMusicViewModel,
    playlistsVM: PlaylistsViewModel,
    timeToPauseVM: TimeToPauseViewModel,
    storageListVM: StorageListViewModel,
) {
    Box(
        modifier = Modifier.background(Color.White),
    ) {
        HorizontalPager(
            state = pagerState
        ) { page ->
            if (page == 0) {
                PlaylistsSubpage(
                    playlistsVM = playlistsVM,
                )
            }
            if (page == 1) {
                DashboardSubpage(timeToPauseVM = timeToPauseVM, storageListVM = storageListVM)
            }
            if (page == 2) {
                SettingSubpage(ctx = ctx)
            }
        }
        BottomBar(
            currentRoute = RoutesKey.HOME,
            bottomBarPageState = pagerState,
            currentMusicVM = currentMusicVM,
        )
    }
}