package com.kutedev.easemusicplayer.singleton

import android.Manifest.permission.READ_EXTERNAL_STORAGE
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.Settings
import androidx.activity.result.ActivityResultLauncher
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton


@Singleton
class PermissionRepository @Inject constructor(
    private val _scope: CoroutineScope
) {
    private var _requestPermissionLauncher: ActivityResultLauncher<String>? = null
    private var _context: Context? = null
    private var _job: Job? = null
    private val _permissionChanged = MutableSharedFlow<Unit>()
    private val _havePermission = MutableStateFlow(false)

    val havePermission = _havePermission.asStateFlow()

    fun onCreate(context: Context, requestPermissionLauncher: ActivityResultLauncher<String>) {
        _requestPermissionLauncher = requestPermissionLauncher
        _context = context
        _job = _scope.launch {
            reloadHaveStoragePermission()
            _permissionChanged.collect {
                reloadHaveStoragePermission()
            }
        }
    }

    fun onDestroy() {
        _job?.cancel()
        _job = null
        _requestPermissionLauncher = null
        _context = null
    }

    fun requestStoragePermission() {
        val cx = _context ?: return

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // All-files access is an appop toggled in system settings, not a
            // runtime dialog: open this app's page on the settings screen.
            // The state is re-evaluated when the activity resumes
            // (`MainActivity.onResume` → `triggerPermissionChanged`).
            val intent = Intent(
                Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                Uri.parse("package:${cx.packageName}"),
            ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            runCatching { cx.startActivity(intent) }
        } else {
            val launcher = this._requestPermissionLauncher ?: return
            launcher.launch(READ_EXTERNAL_STORAGE)
        }
    }

    private fun reloadHaveStoragePermission() {
        _havePermission.value = computeHaveStoragePermission()
    }

    private fun computeHaveStoragePermission(): Boolean {
        val cx = _context ?: return false

        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // True v3 way: the local storage backend reads by raw path, which
            // needs all-files access — READ_MEDIA_AUDIO alone is denied for
            // files contributed by other apps.
            Environment.isExternalStorageManager()
        } else {
            cx.checkSelfPermission(READ_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED
        }
    }

    fun triggerPermissionChanged() {
        _scope.launch {
            _permissionChanged.emit(Unit)
        }
    }
}
