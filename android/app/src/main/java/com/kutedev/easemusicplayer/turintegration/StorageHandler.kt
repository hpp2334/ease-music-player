package com.kutedev.easemusicplayer.turintegration

import com.kutedev.easemusicplayer.singleton.StorageRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Handles `ease.context.notifyChange()` / `ease.context.createStorage()`
 * upcalls fired by plugin instances (views + backends) through
 * [EaseStorageHost]. Reloads the storage list so kv-side changes (an alias
 * rename, or a removal done by the backend) are reflected in the dashboard +
 * the edit page (which observes the list and pops back when its storage
 * disappears); `storageCreated` additionally emits the connected event so a
 * create-mode setup form pops once the instance is persisted.
 */
@Singleton
class StorageHandler @Inject constructor(
    private val storageRepository: StorageRepository,
    private val scope: CoroutineScope,
) {
    fun notifyChange(pluginId: String, pluginStorageId: String) {
        scope.launch {
            storageRepository.reload()
        }
    }

    fun storageCreated(pluginId: String, pluginStorageId: String) {
        scope.launch {
            storageRepository.onPluginStorageCreated()
        }
    }
}
