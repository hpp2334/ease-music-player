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
import androidx.activity.viewModels
import com.kutedev.easemusicplayer.core.BackendService
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.AppVM
import com.kutedev.easemusicplayer.viewmodels.AppVMFactory
import com.kutedev.easemusicplayer.viewmodels.getAppVersion
import dagger.hilt.android.AndroidEntryPoint
import dagger.hilt.android.HiltAndroidApp
import dagger.hilt.android.lifecycle.withCreationCallback

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    private lateinit var bridge: Bridge
    private val _appVM by viewModels<AppVM>(
        extrasProducer = {
            defaultViewModelCreationExtras.withCreationCallback<
                    AppVMFactory> { factory ->
                factory.create(getAppVersion(context = this.applicationContext))
            }
        }
    )

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(null)
        enableEdgeToEdge()

        bridge = Bridge(this);
        bridge.initialize();
        startService(Intent(this, BackendService::class.java))

        setContent {
            Root()
        }
    }

    override fun onStart() {
        super.onStart()
        ensurePostNotificationsPermission()
    }

    override fun onStop() {
        super.onStop()
    }

    override fun onDestroy() {
        super.onDestroy()
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        intent?.data?.let { uri ->
            val code = uri.getQueryParameter("code")
            if (code != null) {
                // TODO: impl
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
class EaseMusicPlayerApplication : Application() {  }