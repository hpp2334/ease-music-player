package com.kutedev.easemusicplayer.singleton

import android.Manifest.permission.READ_EXTERNAL_STORAGE
import android.Manifest.permission.READ_MEDIA_AUDIO
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
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
        val launcher = this._requestPermissionLauncher ?: return

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            launcher.launch(READ_MEDIA_AUDIO)
        } else {
            launcher.launch(READ_EXTERNAL_STORAGE)
        }
    }

    private fun reloadHaveStoragePermission() {
        _havePermission.value = computeHaveStoragePermission()
    }

    private fun computeHaveStoragePermission(): Boolean {
        val cx = _context ?: return false

        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            cx.checkSelfPermission(READ_MEDIA_AUDIO) == PackageManager.PERMISSION_GRANTED
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