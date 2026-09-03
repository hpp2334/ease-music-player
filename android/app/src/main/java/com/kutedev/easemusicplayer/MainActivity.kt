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
import com.kutedev.cantode.Cantode
import com.kutedev.easemusicplayer.core.KeepBackendService
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PermissionRepository
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.PluginOAuthState
import com.kutedev.easemusicplayer.singleton.StorageRepository
import dagger.hilt.android.AndroidEntryPoint
import dagger.hilt.android.HiltAndroidApp
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
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
    @Inject lateinit var pluginOAuthState: PluginOAuthState
    @Inject lateinit var oauthHandler: com.kutedev.easemusicplayer.turintegration.OauthHandler
    @Inject lateinit var storageHandler: com.kutedev.easemusicplayer.turintegration.StorageHandler

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        startService(Intent(this, KeepBackendService::class.java))
        bridge.initialize();
        // Wire the `ease:oauth` + `ease:context` Rust→Kotlin upcall targets
        // so plugin views can trigger OAuth (`ease:oauth.start`) and the
        // edit view can disconnect (`ease:context.disconnect`).
        com.kutedev.easemusicplayer.turintegration.EaseOauthHost.install(oauthHandler)
        com.kutedev.easemusicplayer.turintegration.EaseStorageHost.install(storageHandler)
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
            setupCantodeEngine()
            // Connect the plugin event bus after the player repo is wired
            // so plugins begin receiving music:play / resume / pause / stop /
            // complete.
            pluginRepository.bindPlayerEvents(playerControllerRepository)
        }
    }

    /**
     * Build the cantode player context + player + the [Cantode] facade
     * via [PlayerControllerRepository]. The facade is cantode's own
     * Kotlin half (`:cantode-engine`) and talks to the engine through
     * cantode's JNI bridge under the player handle id; it gets its own
     * CoroutineScope for the 10 Hz poll loop.
     */
    private fun setupCantodeEngine() {
        playerControllerRepository.setupCantodeEngine { playerHandleId ->
            Cantode(
                playerHandle = playerHandleId,
                scope = kotlinx.coroutines.CoroutineScope(
                    kotlinx.coroutines.Dispatchers.Default + SupervisorJob(),
                ),
            )
        }
    }

    private fun setupExceptionHandler() {
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            bridge.logRaw("error", "on uncaught exception: $throwable")
            bridge.logRaw("error", "on uncaught exception stacktrace: ${throwable.stackTraceToString()}")

            android.os.Process.killProcess(android.os.Process.myPid())
            exitProcess(1)
        }
    }

    override fun onStop() {
        super.onStop()
        // No teardown needed: the cantode engine / context live in
        // PlayerControllerRepository (singleton-scoped), not tied to this
        // activity's lifetime. PlaybackService still owns the MediaSession.
    }

    override fun onDestroy() {
        super.onDestroy()

        permissionRepository.onDestroy()
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        // OneDrive (and other JS plugin) OAuth redirect, e.g.
        // `easem://oauth2redirect/?code=...`. Take the pending
        // (pluginId, oauthId) stashed when the browser was launched,
        // exchange the code via the plugin (which consumes its own
        // `oauth:<oauthId>` pending slot), and reload the storage list.
        val data = intent?.data
        val code = data?.getQueryParameter("code")
        if (code.isNullOrBlank()) return
        val pending = pluginOAuthState.take() ?: return
        lifecycleScope.launch {
            val id = storageRepository.pluginOAuthExchange(pending.first, pending.second, code)
            if (id != null) {
                bridge.logRaw("info", "plugin OAuth connected: plugin=${pending.first} id=$id")
            } else {
                bridge.logRaw("error", "plugin OAuth exchange failed: plugin=${pending.first}")
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
