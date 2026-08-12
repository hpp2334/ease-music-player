package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import com.kutedev.easemusicplayer.singleton.types.ArgStoragePluginOauthExchange
import com.kutedev.easemusicplayer.singleton.types.ArgStoragePluginProvider
import com.kutedev.easemusicplayer.singleton.types.ArgUpsertWebdavStorage
import com.kutedev.easemusicplayer.singleton.types.PluginOauthExchangeResult
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
    private val _pluginConnectedEvent = MutableSharedFlow<StorageId>()
    private val _pluginDisconnectedEvent = MutableSharedFlow<StorageId>()

    val storages = _storages.asStateFlow()
    val preRemoveStorageEvent = _preRemoveStorageEvent.asSharedFlow()
    val onRemoveStorageEvent = _onRemoveStorageEvent.asSharedFlow()
    /** Emitted when a JS plugin OAuth exchange mints a new storage row.
     *  `EditStoragesPage` collects this to pop back from the setup form. */
    val pluginConnectedEvent = _pluginConnectedEvent.asSharedFlow()
    /** Emitted when a plugin instance is disconnected (via `ease.context.disconnect()`
     *  from an edit view). `EditStoragesPage` collects this to pop back. */
    val pluginDisconnectedEvent = _pluginDisconnectedEvent.asSharedFlow()

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

    // === JS plugin storage providers (e.g. OneDrive) ========================

    /** Ask the plugin for its OAuth authorization URL (`<provider>:oauth.url`). */
    suspend fun pluginOAuthUrl(provider: String): String? =
        bridge.call(BridgeMethods.StoragePlugin.OAUTH_URL, ArgStoragePluginProvider(provider))
            .unwrapOrNull()?.payload?.url

    /**
     * Complete OAuth: exchange the authorization `code` for tokens via the
     * plugin (`<provider>:oauth.exchange`), which mints + persists a new
     * instance, then register the storage row. Returns the new storage id.
     */
    suspend fun pluginOAuthExchange(provider: String, code: String, alias: String?): StorageId? {
        val result: PluginOauthExchangeResult? =
            bridge.call(
                BridgeMethods.StoragePlugin.OAUTH_EXCHANGE,
                ArgStoragePluginOauthExchange(provider = provider, code = code, alias = alias),
            ).unwrapOrNull()?.payload
        if (result != null) {
            reload()
            _pluginConnectedEvent.emit(result.storageId)
        }
        return result?.storageId
    }

    /** Remove a plugin storage AND tell the plugin to drop its config/secret. */
    suspend fun pluginRemoveInstance(id: StorageId) {
        _preRemoveStorageEvent.emit(id)
        bridge.call(BridgeMethods.StoragePlugin.REMOVE_INSTANCE, id).unwrapOrNull()
        _onRemoveStorageEvent.emit(Unit)
        _pluginDisconnectedEvent.emit(id)
        reload()
    }

    /**
     * Look up a plugin storage row by `(pluginId, pluginStorageId)`. Returns
     * the storage id, or `null` if no such row exists. Drives
     * `ease.context.disconnect()` resolution from the plugin edit view.
     */
    fun findPluginStorage(pluginId: String, pluginStorageId: String): StorageId? {
        return _storages.value.firstOrNull { s ->
            val h = s.handle
            h is StorageHandle.Plugin &&
                h.pluginId.id == pluginId &&
                h.pluginStorageId.id == pluginStorageId
        }?.id
    }

    suspend fun reload() {
        val list: List<Storage>? = bridge.call(BridgeMethods.Storage.LIST).unwrapOrNull()?.payload
        _storages.value = list ?: emptyList()
    }
}

/**
 * Holds the in-flight plugin OAuth flow state between launching the browser
 * and the `easem://oauth2redirect` callback reaching `MainActivity.onNewIntent`.
 * Set when the user taps "Authorize", consumed when the redirect lands.
 */
@Singleton
class PluginOAuthState @Inject constructor() {
    @Volatile var provider: String? = null
        private set
    @Volatile var alias: String? = null
        private set

    fun set(provider: String, alias: String?) {
        this.provider = provider
        this.alias = alias
    }

    fun take(): Pair<String, String?>? {
        val p = provider ?: return null
        val a = alias
        provider = null
        alias = null
        return p to a
    }
}
