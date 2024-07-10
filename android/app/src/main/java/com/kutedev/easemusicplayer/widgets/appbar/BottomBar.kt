package com.kutedev.easemusicplayer.widgets.appbar

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.pager.PagerState
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.res.colorResource
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.navigation.compose.currentBackStackEntryAsState
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.Routes
import com.kutedev.easemusicplayer.viewmodels.RootSubkeyViewModel
import kotlinx.coroutines.launch
import uniffi.ease_client.RootRouteSubKey
import uniffi.ease_client.VRootSubKeyState

private interface IBottomItem {
    val painterId: Int;
    val pageIndex: Int;
}

private object BPlaylist: IBottomItem {
    override val painterId: Int
        get() = R.drawable.icon_album
    override val pageIndex: Int
        get() = 0;
}

private object BDashboard: IBottomItem {
    override val painterId: Int
        get() = R.drawable.icon_dashboard
    override val pageIndex: Int
        get() = 1;
}

private object BSetting: IBottomItem {
    override val painterId: Int
        get() = R.drawable.icon_setting
    override val pageIndex: Int
        get() = 2;
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun BottomBar(bottomBarPageState: PagerState) {
    val items = listOf(
        BPlaylist,
        BDashboard,
        BSetting
    )
    val animationScope = rememberCoroutineScope()
    val currentRouteState = LocalNavController.current.currentBackStackEntryAsState().value;

    if (currentRouteState?.destination?.route != Routes.Home) {
        return;
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(60.dp)
    ) {
        for (item in items) {
            val isSelected = bottomBarPageState.currentPage == item.pageIndex;
            val tint = if (isSelected) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.surfaceVariant
            }

            Box(modifier = Modifier
                .weight(1.0f)
                .fillMaxHeight()
                .align(Alignment.CenterVertically)
                .clickable {
                    animationScope.launch {
                        bottomBarPageState.animateScrollToPage(item.pageIndex);
                    }
                }) {
                Icon(
                    painter = painterResource(id = item.painterId),
                    tint = tint,
                    contentDescription = null,
                    modifier = Modifier
                        .width(20.dp)
                        .height(20.dp)
                        .align(Alignment.Center)
                )
            }
        }
    }
}