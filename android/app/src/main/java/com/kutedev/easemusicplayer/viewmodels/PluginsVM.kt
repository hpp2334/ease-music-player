package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.singleton.PluginViewItem
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject

@HiltViewModel
class PluginsVM @Inject constructor(
    private val pluginRepository: PluginRepository,
) : ViewModel() {
    val pluginViews: kotlinx.coroutines.flow.StateFlow<List<PluginViewItem>> =
        pluginRepository.pluginViews
}
