package com.kutedev.easemusicplayer.widgets.appbar

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.pager.PagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.navigation.compose.currentBackStackEntryAsState
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.dropShadow
import com.kutedev.easemusicplayer.viewmodels.EaseViewModel
import com.kutedev.easemusicplayer.widgets.musics.MiniPlayer
import kotlinx.coroutines.launch
import uniffi.ease_client.RoutesKey
import kotlin.math.tan

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

fun getBottomBarSpace(isPlaying: Boolean): Dp {
    var total = 60.dp;
    if (isPlaying) {
        total += 124.dp;
    }
    return total;
}

@Composable
fun BottomBarSpacer(
    hasCurrentMusic: Boolean
) {
    Box(modifier = Modifier.height(getBottomBarSpace(hasCurrentMusic)))
}

@Composable
fun BoxScope.BottomBar(
    currentRoute: RoutesKey,
    bottomBarPageState: PagerState?,
    evm: EaseViewModel,
    scaffoldPadding: PaddingValues,
) {
    val items = listOf(
        BPlaylist,
        BDashboard,
        BSetting
    )
    val animationScope = rememberCoroutineScope()

    val hasCurrentMusic = evm.currentMusicState.collectAsState().value.id != null

    val showBottomBar = currentRoute == RoutesKey.HOME
    val showMiniPlayer = hasCurrentMusic && (currentRoute == RoutesKey.HOME || currentRoute == RoutesKey.PLAYLIST)

    if (!showBottomBar && !showMiniPlayer) {
        Box(modifier = Modifier
            .fillMaxWidth()
            .height(scaffoldPadding.calculateBottomPadding())
        )
        return;
    }

    Column(
        modifier = Modifier
            .align(Alignment.BottomStart)
            .dropShadow(
                MaterialTheme.colorScheme.surfaceVariant,
                0.dp,
                (-4).dp,
                8.dp,
            )
            .clip(RoundedCornerShape(topStart = 12.dp, topEnd = 12.dp, bottomStart = 0.dp, bottomEnd = 0.dp))
            .background(Color.White)
            .fillMaxWidth()
    ) {
        if (showMiniPlayer) {
            MiniPlayer(
                evm = evm
            )
        }
        if (showBottomBar && bottomBarPageState != null) {
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
        Box(modifier = Modifier
            .fillMaxWidth()
            .height(scaffoldPadding.calculateBottomPadding())
        )
    }
}