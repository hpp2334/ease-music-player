package com.kutedev.easemusicplayer

import androidx.compose.animation.core.tween
import androidx.compose.animation.slideIn
import androidx.compose.animation.slideOut
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.LayoutDirection
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import com.kutedev.easemusicplayer.ui.theme.EaseMusicPlayerTheme
import com.kutedev.easemusicplayer.widgets.LocalNavController
import com.kutedev.easemusicplayer.widgets.RouteHome
import com.kutedev.easemusicplayer.widgets.RoutesProvider
import com.kutedev.easemusicplayer.widgets.home.HomePage
import com.kutedev.easemusicplayer.widgets.playlists.CreatePlaylistsDialog

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
                            startDestination = RouteHome,
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
                            composable<RouteHome> {
                                HomePage(
                                    scaffoldPadding = scaffoldPadding,
                                )
                                CreatePlaylistsDialog()
                            }
                            composable(RoutesKey.ADD_DEVICES.value) {
//                                EditStoragesPage(
//                                    evm = evm,
//                                )
                            }
                            composable(RoutesKey.PLAYLIST.value) {
//                                PlaylistPage(
//                                    evm = evm,
//                                    scaffoldPadding = scaffoldPadding,
//                                )
//                                EditPlaylistsDialog(evm = evm)
                            }
                            composable(RoutesKey.IMPORT_MUSICS.value) {
//                                ImportMusicsPage(evm = evm)
                            }
                            composable(RoutesKey.MUSIC_PLAYER.value) {
//                                MusicPlayerPage(
//                                    evm = evm
//                                )
                            }
                        }
//                        TimeToPauseModal(evm = evm)
                    }
                }
            }
        }
    }
}