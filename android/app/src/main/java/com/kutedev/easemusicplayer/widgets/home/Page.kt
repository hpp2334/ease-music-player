package com.kutedev.easemusicplayer.widgets.home

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.runtime.Composable
import com.kutedev.easemusicplayer.viewmodels.PlaylistsViewModel
import com.kutedev.easemusicplayer.viewmodels.StorageListViewModel
import com.kutedev.easemusicplayer.viewmodels.TimeToPauseViewModel
import com.kutedev.easemusicplayer.widgets.dashboard.DashboardSubpage
import com.kutedev.easemusicplayer.widgets.playlists.PlaylistsSubpage
import com.kutedev.easemusicplayer.widgets.settings.SettingSubpage

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun HomePage(
    ctx: android.content.Context,
    pagerState: PagerState,
    playlistsVM: PlaylistsViewModel,
    timeToPauseVM: TimeToPauseViewModel,
    storageListVM: StorageListViewModel,
) {
    HorizontalPager(state = pagerState) { page ->
        if (page == 0) {
            PlaylistsSubpage(playlistsVM = playlistsVM)
        }
        if (page == 1) {
            DashboardSubpage(timeToPauseVM = timeToPauseVM, storageListVM = storageListVM)
        }
        if (page == 2) {
            SettingSubpage(ctx = ctx)
        }
    }
}