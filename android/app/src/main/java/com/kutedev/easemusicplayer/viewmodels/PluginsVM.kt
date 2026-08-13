package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.singleton.PluginViewItem
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class PluginsVM @Inject constructor(
    private val pluginRepository: PluginRepository,
) : ViewModel() {
    val pluginViews: kotlinx.coroutines.flow.StateFlow<List<PluginViewItem>> =
        pluginRepository.pluginViews

    init {
        // Ensure the manifest scan has run (the backend service also calls
        // scanPlugins at startup; this covers the Plugins tab being opened
        // before that coroutine lands).
        viewModelScope.launch {
            pluginRepository.scanPlugins()
        }
    }
}
