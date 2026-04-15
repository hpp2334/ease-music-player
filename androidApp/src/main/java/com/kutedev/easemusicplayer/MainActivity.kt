package com.kutedev.easemusicplayer

import android.Manifest.permission.POST_NOTIFICATIONS
import android.app.Application
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors
import com.kutedev.easemusicplayer.core.KeepBackendService
import com.kutedev.easemusicplayer.core.PlaybackService
import com.kutedev.easemusicplayer.di.appModule
import com.kutedev.easemusicplayer.platform.appContext
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PermissionRepository
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import kotlinx.coroutines.launch
import org.koin.android.ext.android.inject
import org.koin.core.context.GlobalContext.startKoin
import uniffi.ease_client_backend.easeLog
import kotlin.system.exitProcess

class MainActivity : ComponentActivity() {
    private val bridge: Bridge by inject()
    private val storageRepository: StorageRepository by inject()
    private val playlistRepository: PlaylistRepository by inject()
    private val playerControllerRepository: PlayerControllerRepository by inject()
    private val playerRepository: PlayerRepository by inject()
    private val permissionRepository: PermissionRepository by inject()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        startService(Intent(this, KeepBackendService::class.java))
        bridge.initialize()
        setupExceptionHandler()

        val requestPermissionLauncher = registerForActivityResult(ActivityResultContracts.RequestPermission()) {
            _ -> permissionRepository.triggerPermissionChanged()
        }
        permissionRepository.onCreate(this, requestPermissionLauncher)

        setContent {
            Root()
        }
    }

    override fun onStart() {
        super.onStart()
        ensurePostNotificationsPermission()

        lifecycleScope.launch {
            playerRepository.reload()
            storageRepository.reload()
            playlistRepository.reload()
            setupMediaController()
        }
    }

    private fun setupMediaController() {
        val factory = MediaController.Builder(
            this,
            SessionToken(this, ComponentName(this, PlaybackService::class.java))
        ).buildAsync()
        factory.addListener(
            {
                factory.let {
                    if (it.isDone) {
                        val controller = it.get()
                        playerControllerRepository.setupMediaController(controller)
                        controller
                    } else {
                        null
                    }
                }
            },
            MoreExecutors.directExecutor()
        )
    }

    private fun setupExceptionHandler() {
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            easeLog("on uncaught exception: $throwable")
            easeLog("on uncaught exception stacktrace: ${throwable.stackTraceToString()}")

            android.os.Process.killProcess(android.os.Process.myPid())
            exitProcess(1)
        }
    }

    override fun onStop() {
        super.onStop()
        playerControllerRepository.destroyMediaController()
    }

    override fun onDestroy() {
        super.onDestroy()

        permissionRepository.onDestroy()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        intent.data?.let { uri ->
            val code = uri.getQueryParameter("code")
            if (code != null) {
                lifecycleScope.launch {
                    storageRepository.updateRefreshToken(code)
                }
            }
        }
    }

    private fun ensurePostNotificationsPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (checkSelfPermission(
                    POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                requestPermissions(
                    arrayOf(POST_NOTIFICATIONS),
                    101
                )
            }
        }
    }
}

class EaseMusicPlayerApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        appContext = this
        startKoin {
            modules(appModule)
        }
    }
}
