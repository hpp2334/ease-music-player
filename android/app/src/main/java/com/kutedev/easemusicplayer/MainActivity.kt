package com.kutedev.easemusicplayer

import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.ServiceConnection
import android.os.Bundle
import android.os.IBinder
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.localbroadcastmanager.content.LocalBroadcastManager
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import com.kutedev.easemusicplayer.core.BACKEND_STARTED_ACTION
import com.kutedev.easemusicplayer.core.IOnNotifyView
import com.kutedev.easemusicplayer.core.UIBridge
import com.kutedev.easemusicplayer.core.UIBridgeController
import com.kutedev.easemusicplayer.ui.theme.EaseMusicPlayerTheme
import com.kutedev.easemusicplayer.viewmodels.EaseViewModel
import com.kutedev.easemusicplayer.widgets.RoutesProvider
import com.kutedev.easemusicplayer.widgets.dashboard.TimeToPauseModal
import com.kutedev.easemusicplayer.widgets.devices.EditStoragesPage
import com.kutedev.easemusicplayer.widgets.home.HomePage
import com.kutedev.easemusicplayer.widgets.musics.ImportMusicsPage
import com.kutedev.easemusicplayer.widgets.musics.MusicPlayerPage
import com.kutedev.easemusicplayer.widgets.playlists.PlaylistPage
import com.kutedev.easemusicplayer.widgets.playlists.CreatePlaylistsDialog
import com.kutedev.easemusicplayer.widgets.playlists.EditPlaylistsDialog
import uniffi.ease_client.MainAction
import uniffi.ease_client.RoutesKey
import uniffi.ease_client.ViewAction

inline fun <reified T> MainActivity.registerViewModel(bridge: UIBridge)
where T : ViewModel, T : IOnNotifyView {
    val vm: T by viewModels()
    bridge.registerView(vm)

    vmDestroyers.add {
        bridge.unregisterView(vm)
    }
}

class MainActivity : ComponentActivity() {
    val vmDestroyers = mutableListOf<() -> Unit>()
    var uiBridge: UIBridge? = null
    private val uiReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == BACKEND_STARTED_ACTION) {
                uiBridge?.onBackendConnected()
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        LocalBroadcastManager.getInstance(this).registerReceiver(uiReceiver, IntentFilter(
            BACKEND_STARTED_ACTION))

        val requestPermissionLauncher = registerForActivityResult(ActivityResultContracts.RequestPermission()) { _ ->
            uiBridge!!.schedule {
                uiBridge!!.dispatchAction(ViewAction.Main(MainAction.PERMISSION_CHANGED))
            }
        }
        uiBridge = UIBridge()
        uiBridge!!.onActivityCreate(applicationContext, requestPermissionLauncher)
        registerViewModels()

        setContent {
            Root()
        }
    }

    override fun onStart() {
        super.onStart()
        uiBridge!!.onActivityStart()
    }

    override fun onStop() {
        super.onStop()
        uiBridge!!.onActivityStop()
    }

    override fun onDestroy() {
        super.onDestroy()

        for (destroy in vmDestroyers) {
            destroy()
        }
        vmDestroyers.clear()
        uiBridge!!.onActivityDestroy()
        LocalBroadcastManager.getInstance(this).unregisterReceiver(uiReceiver)
    }

    @Composable
    fun Root() {
        val evm: EaseViewModel by viewModels()

        CompositionLocalProvider(UIBridgeController provides uiBridge!!) {
            RoutesProvider { controller ->
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
                            val bottomBarPageState = rememberPagerState(pageCount = {
                                3
                            })

                            Box(
                                modifier = Modifier.weight(1f)
                            ) {
                                NavHost(
                                    modifier = Modifier
                                        .fillMaxSize(),
                                    navController = controller,
                                    startDestination = RoutesKey.HOME.toString(),
                                    enterTransition = { EnterTransition.None },
                                    exitTransition = { ExitTransition.None },
                                    popEnterTransition = { EnterTransition.None },
                                    popExitTransition = { ExitTransition.None },
                                ) {
                                    composable(RoutesKey.HOME.toString()) {
                                        HomePage(
                                            ctx = applicationContext,
                                            pagerState = bottomBarPageState,
                                            evm = evm,
                                            scaffoldPadding = scaffoldPadding,
                                        )
                                        CreatePlaylistsDialog(evm = evm)
                                    }
                                    composable(RoutesKey.ADD_DEVICES.toString()) {
                                        EditStoragesPage(
                                            evm = evm,
                                        )
                                    }
                                    composable(RoutesKey.PLAYLIST.toString()) {
                                        PlaylistPage(
                                            evm = evm,
                                            scaffoldPadding = scaffoldPadding,
                                        )
                                        EditPlaylistsDialog(evm = evm)
                                    }
                                    composable(RoutesKey.IMPORT_MUSICS.toString()) {
                                        ImportMusicsPage(evm = evm)
                                    }
                                    composable(RoutesKey.MUSIC_PLAYER.toString()) {
                                        MusicPlayerPage(
                                            evm = evm
                                        )
                                    }
                                }
                                TimeToPauseModal(evm = evm)
                            }
                        }
                    }
                }
            }
        }
    }

    private fun registerViewModels() {
        registerViewModel<EaseViewModel>(uiBridge!!)
    }
}

