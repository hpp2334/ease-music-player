package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import com.kutedev.easemusicplayer.singleton.types.ArgUpsertWebdavStorage
import com.kutedev.easemusicplayer.singleton.types.Storage
import com.kutedev.easemusicplayer.singleton.types.StorageId
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class StorageRepository @Inject constructor(
    private val bridge: Bridge,
    private val scope: CoroutineScope,
) {
    private val _storages = MutableStateFlow(listOf<Storage>())
    private val _preRemoveStorageEvent = MutableSharedFlow<StorageId>()
    private val _onRemoveStorageEvent = MutableSharedFlow<Unit>()

    val storages = _storages.asStateFlow()
    val preRemoveStorageEvent = _preRemoveStorageEvent.asSharedFlow()
    val onRemoveStorageEvent = _onRemoveStorageEvent.asSharedFlow()

    suspend fun createStorage(arg: ArgUpsertWebdavStorage): Boolean {
        require(arg.id == null) { "createStorage: arg.id must be null" }
        if (bridge.call(BridgeMethods.StorageWebdav.UPSERT, arg).unwrapOrNull() == null) return false
        reload()
        return true
    }

    suspend fun updateStorage(arg: ArgUpsertWebdavStorage): Boolean {
        require(arg.id != null) { "updateStorage: arg.id must be non-null" }
        if (bridge.call(BridgeMethods.StorageWebdav.UPSERT, arg).unwrapOrNull() == null) return false
        reload()
        return true
    }

    suspend fun testStorage(arg: ArgUpsertWebdavStorage) =
        bridge.call(BridgeMethods.StorageWebdav.TEST, arg).unwrapOrNull()?.payload

    suspend fun remove(id: StorageId) {
        _preRemoveStorageEvent.emit(id)
        bridge.call(BridgeMethods.Storage.REMOVE, id).unwrapOrNull()
        _onRemoveStorageEvent.emit(Unit)
        reload()
    }

    suspend fun reload() {
        val list: List<Storage>? = bridge.call(BridgeMethods.Storage.LIST).unwrapOrNull()?.payload
        _storages.value = list ?: emptyList()
    }
}
