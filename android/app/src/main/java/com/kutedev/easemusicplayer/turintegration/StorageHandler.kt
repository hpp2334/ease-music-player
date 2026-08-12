package com.kutedev.easemusicplayer.turintegration

import com.kutedev.easemusicplayer.singleton.StorageRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Handles `ease.context.disconnect()` upcalls fired by plugin edit views
 * through [EaseStorageHost]. Resolves the storage row by
 * `(pluginId, pluginStorageId)` and invokes
 * [StorageRepository.pluginRemoveInstance]; the resulting
 * `pluginDisconnectedEvent` lets the host-side UI (`EditStoragesPage`)
 * pop back from the edit form.
 */
@Singleton
class StorageHandler @Inject constructor(
    private val storageRepository: StorageRepository,
    private val scope: CoroutineScope,
) {
    fun disconnect(pluginId: String, pluginStorageId: String) {
        scope.launch {
            val id = storageRepository.findPluginStorage(pluginId, pluginStorageId) ?: run {
                android.util.Log.w(
                    "StorageHandler",
                    "disconnect: no storage row for ($pluginId, $pluginStorageId)",
                )
                return@launch
            }
            storageRepository.pluginRemoveInstance(id)
        }
    }
}
