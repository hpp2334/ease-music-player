package com.kutedev.easemusicplayer

import android.Manifest.permission.POST_NOTIFICATIONS
import android.app.Application
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.NotificationCompat
import androidx.lifecycle.lifecycleScope
import com.kutedev.easemusicplayer.core.CantodePlayer
import com.kutedev.easemusicplayer.core.KeepBackendService
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PermissionRepository
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import dagger.hilt.android.AndroidEntryPoint
import dagger.hilt.android.HiltAndroidApp
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.easeLog
import javax.inject.Inject
import kotlin.system.exitProcess

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    @Inject lateinit var bridge: Bridge
    @Inject lateinit var storageRepository: StorageRepository
    @Inject lateinit var playlistRepository: PlaylistRepository
    @Inject lateinit var playerControllerRepository: PlayerControllerRepository
    @Inject lateinit var playerRepository: PlayerRepository
    @Inject lateinit var permissionRepository: PermissionRepository
    @Inject lateinit var pluginRepository: com.kutedev.easemusicplayer.singleton.PluginRepository

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        startService(Intent(this, KeepBackendService::class.java))
        bridge.initialize();
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
            setupCantodePlayer()
            // Connect the plugin event bus after the player repo is wired
            // so plugins begin receiving music:play / pause / stop / complete.
            pluginRepository.bindPlayerEvents(playerControllerRepository)
        }
    }

    /**
     * Build the cantode PlayerContextHandle + PlayerHandle + CantodePlayer
     * via [PlayerControllerRepository]. The CantodePlayer gets its own
     * CoroutineScope for the 10Hz state poll loop.
     */
    private fun setupCantodePlayer() {
        playerControllerRepository.setupCantodePlayer { handle ->
            CantodePlayer(
                playerRepository = playerRepository,
                handle = handle,
                scope = kotlinx.coroutines.CoroutineScope(
                    kotlinx.coroutines.Dispatchers.Default + SupervisorJob(),
                ),
            )
        }
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
        // No teardown needed: the cantode player / context live in
        // PlayerControllerRepository (singleton-scoped), not tied to this
        // activity's lifetime. PlaybackService still owns the MediaSession.
    }

    override fun onDestroy() {
        super.onDestroy()

        permissionRepository.onDestroy()
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        intent?.data?.let { uri ->
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

@HiltAndroidApp
class EaseMusicPlayerApplication : Application() {
    companion object {
        init {
            System.loadLibrary("ease_client_backend")
        }
    }

    /**
     * JNI hook for cpal's AAudio backend: register the JavaVM + app Context
     * into ndk-context. MUST be called before any cpal interaction.
     * Idempotent on the Rust side (OnceLock guarded).
     */
    private external fun nativeInitAndroidContext(context: android.content.Context)

    override fun onCreate() {
        super.onCreate()
        nativeInitAndroidContext(this)
    }
}