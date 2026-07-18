package com.kutedev.easemusicplayer

import androidx.activity.ComponentActivity
import androidx.compose.animation.core.tween
import androidx.compose.animation.slideIn
import androidx.compose.animation.slideOut
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.LayoutDirection
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.kutedev.easemusicplayer.ui.theme.EaseMusicPlayerTheme
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RouteAddDevices
import com.kutedev.easemusicplayer.core.RouteDebugMore
import com.kutedev.easemusicplayer.core.RouteHome
import com.kutedev.easemusicplayer.core.RouteImport
import com.kutedev.easemusicplayer.core.RouteLog
import com.kutedev.easemusicplayer.core.RouteMusicPlayer
import com.kutedev.easemusicplayer.core.RoutePlaylist
import com.kutedev.easemusicplayer.core.RoutesProvider
import com.kutedev.easemusicplayer.viewmodels.EditStorageVM
import com.kutedev.easemusicplayer.widgets.ToastFrame
import com.kutedev.easemusicplayer.widgets.dashboard.TimeToPauseModal
import com.kutedev.easemusicplayer.widgets.devices.EditStoragesPage
import com.kutedev.easemusicplayer.widgets.home.HomePage
import com.kutedev.easemusicplayer.widgets.musics.ImportMusicsPage
import com.kutedev.easemusicplayer.widgets.musics.MusicPlayerPage
import com.kutedev.easemusicplayer.widgets.playlists.CreatePlaylistsDialog
import com.kutedev.easemusicplayer.widgets.playlists.EditPlaylistsDialog
import com.kutedev.easemusicplayer.widgets.playlists.PlaylistPage
import com.kutedev.easemusicplayer.widgets.settings.DebugMorePage
import com.kutedev.easemusicplayer.widgets.settings.LogPage

@Composable
fun Root() {
    RoutesProvider {
        val controller = LocalNavController.current

        EaseMusicPlayerTheme {
            Scaffold(
                modifier = Modifier.fillMaxSize(),
            ) { scaffoldPadding ->
                Column(
                    modifier = Modifier
                        .padding(
                            start = scaffoldPadding.calculateLeftPadding(LayoutDirection.Ltr),
                            end = scaffoldPadding.calculateRightPadding(LayoutDirection.Ltr),
                            top = scaffoldPadding.calculateTopPadding(),
                        )
                        .fillMaxSize()
                ) {
                    Box(
                        modifier = Modifier.weight(1f)
                    ) {
                        NavHost(
                            modifier = Modifier
                                .fillMaxSize(),
                            navController = controller,
                            startDestination = RouteHome(),
                            enterTransition = {
                                slideIn(
                                    animationSpec = tween(300),
                                    initialOffset = { fullSize ->
                                        IntOffset(fullSize.width, 0)
                                    })
                            },
                            exitTransition = {
                                slideOut(
                                    animationSpec = tween(300),
                                    targetOffset = { fullSize ->
                                        IntOffset(-fullSize.width, 0)
                                    })
                            },
                            popEnterTransition = {
                                slideIn(
                                    animationSpec = tween(300),
                                    initialOffset = { fullSize ->
                                        IntOffset(fullSize.width, 0)
                                    })
                            },
                            popExitTransition = {
                                slideOut(
                                    animationSpec = tween(300),
                                    targetOffset = { fullSize ->
                                        IntOffset(-fullSize.width, 0)
                                    })
                            },
                        ) {
                            composable(RouteHome()) {
                                HomePage(
                                    scaffoldPadding = scaffoldPadding,
                                )
                                CreatePlaylistsDialog()
                                TimeToPauseModal()
                            }
                            composable(
                                RouteAddDevices("{id}"),
                                arguments = listOf(navArgument("id") { type = NavType.LongType })
                            ) {
                                EditStoragesPage()
                            }
                            composable(
                                RoutePlaylist("{id}"),
                                arguments = listOf(navArgument("id") { type = NavType.LongType })
                            ) {
                                PlaylistPage(
                                    scaffoldPadding = scaffoldPadding,
                                )
                                EditPlaylistsDialog()
                            }
                            composable(
                                RouteImport("{type}"),
                                arguments = listOf(navArgument("type") { type = NavType.StringType } )
                            ){
                                ImportMusicsPage()
                            }
                            composable(RouteMusicPlayer()) {
                                MusicPlayerPage()
                                TimeToPauseModal()
                            }
                            composable(RouteLog()) {
                                LogPage()
                            }
                            composable(RouteDebugMore()) {
                                DebugMorePage()
                            }
                        }
                    }
                    ToastFrame()
                }
            }
        }
    }
}