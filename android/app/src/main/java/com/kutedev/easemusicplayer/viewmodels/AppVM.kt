package com.kutedev.easemusicplayer.viewmodels

import android.content.pm.PackageManager
import android.os.Build
import androidx.lifecycle.ViewModel
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel


fun getAppVersion(
    context: android.content.Context,
): String {
    val packageManager = context.packageManager
    val packageName = context.packageName
    val packageInfo = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        packageManager.getPackageInfo(packageName, PackageManager.PackageInfoFlags.of(0))
    } else {
        packageManager.getPackageInfo(packageName, 0)
    }
    return packageInfo.versionName ?: "<unknown>"
}

@AssistedFactory
interface AppVMFactory {
    fun create(appVersion: String): AppVM
}

@HiltViewModel(assistedFactory = AppVMFactory::class)
class AppVM @AssistedInject constructor(
    @Assisted val appVersion: String
) : ViewModel() {
}
