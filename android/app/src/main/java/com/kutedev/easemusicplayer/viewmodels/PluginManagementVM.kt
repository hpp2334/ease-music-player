package com.kutedev.easemusicplayer.viewmodels

import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.singleton.PluginManager
import com.kutedev.easemusicplayer.singleton.PluginManifest
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import com.kutedev.easemusicplayer.singleton.ToastRepository
import com.kutedev.easemusicplayer.singleton.types.Storage
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/** Backs the Plugin Management page: installed list (enable/disable /
 *  uninstall), storages on each plugin (uninstall warning), and the SAF
 *  install-from-zip path. */
@HiltViewModel
class PluginManagementVM @Inject constructor(
    private val pluginManager: PluginManager,
    private val pluginRepository: PluginRepository,
    private val toastRepository: ToastRepository,
    storageRepository: StorageRepository,
) : ViewModel() {
    val installedPlugins: StateFlow<List<PluginManifest>> = pluginRepository.installedPlugins
    val storages: StateFlow<List<Storage>> = storageRepository.storages

    /** Plugin ids with a mutation in flight (drives row spinners). */
    private val _busyIds = MutableStateFlow<Set<String>>(emptySet())
    val busyIds = _busyIds.asStateFlow()

    init {
        viewModelScope.launch { pluginRepository.scanPlugins() }
    }

    /** How many live storages depend on [pluginId]. */
    fun storageCountFor(pluginId: String, storages: List<Storage>): Int = storages.count {
        (it.handle as? com.kutedev.easemusicplayer.singleton.types.StorageHandle.Plugin)?.pluginId?.id == pluginId
    }

    fun setEnabled(pluginId: String, enabled: Boolean) {
        viewModelScope.launch {
            _busyIds.value = _busyIds.value + pluginId
            try {
                pluginManager.setEnabled(pluginId, enabled)
            } finally {
                _busyIds.value = _busyIds.value - pluginId
            }
        }
    }

    fun uninstall(pluginId: String) {
        viewModelScope.launch {
            _busyIds.value = _busyIds.value + pluginId
            try {
                pluginManager.uninstall(pluginId)
            } finally {
                _busyIds.value = _busyIds.value - pluginId
            }
        }
    }

    fun installFromUri(uri: Uri) {
        viewModelScope.launch {
            _busyIds.value = _busyIds.value + "sideload"
            try {
                pluginManager.installFromUri(uri).fold(
                    onSuccess = { toastRepository.emitToastRes(R.string.plugin_install_ok) },
                    onFailure = { toastRepository.emitToastRes(R.string.plugin_install_fail) },
                )
            } finally {
                _busyIds.value = _busyIds.value - "sideload"
            }
        }
    }
}
