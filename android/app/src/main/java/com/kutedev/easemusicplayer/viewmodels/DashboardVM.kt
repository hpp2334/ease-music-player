package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.DashboardItem
import com.kutedev.easemusicplayer.singleton.PluginManifest
import com.kutedev.easemusicplayer.singleton.PluginRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Backs the Dashboard page: storage list, enabled plugin manifests (for
 *  the "storage removed" status) and plugin dashboard cards. */
@HiltViewModel
class DashboardVM @Inject constructor(
    private val pluginRepository: PluginRepository,
) : ViewModel() {
    val dashboardItems: StateFlow<List<DashboardItem>> = pluginRepository.dashboardItems
    val enabledPlugins: StateFlow<List<PluginManifest>> = pluginRepository.enabledPlugins

    init {
        // Ensure the manifest scan has run (the backend service also calls
        // scanPlugins at startup; this covers the Dashboard being opened
        // before that coroutine lands).
        viewModelScope.launch {
            pluginRepository.scanPlugins()
        }
    }
}
