package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import com.kutedev.easemusicplayer.singleton.types.ArgStoragePluginOauthExchange
import com.kutedev.easemusicplayer.singleton.types.ArgStoragePluginProvider
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
    private val _pluginConnectedEvent = MutableSharedFlow<Unit>()

    val storages = _storages.asStateFlow()
    val preRemoveStorageEvent = _preRemoveStorageEvent.asSharedFlow()
    val onRemoveStorageEvent = _onRemoveStorageEvent.asSharedFlow()
    /** Emitted when a JS plugin registers a new storage instance — either a
     *  non-OAuth backend (`ease.context.createStorage`, e.g. WebDAV's
     *  `webdav:connect`) or an OAuth exchange (`pluginOAuthExchange`).
     *  `EditStoragesPage` collects this to pop back from the setup form. */
    val pluginConnectedEvent = _pluginConnectedEvent.asSharedFlow()

    suspend fun remove(id: StorageId) {
        _preRemoveStorageEvent.emit(id)
        bridge.call(BridgeMethods.Storage.REMOVE, id).unwrapOrNull()
        _onRemoveStorageEvent.emit(Unit)
        reload()
    }

    // === JS plugin storage providers (OneDrive, WebDAV, ...) ===============

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
            _pluginConnectedEvent.emit(Unit)
        }
        return result?.storageId
    }

    /** Remove a plugin storage AND tell the plugin to drop its config/secret. */
    suspend fun pluginRemoveInstance(id: StorageId) {
        _preRemoveStorageEvent.emit(id)
        bridge.call(BridgeMethods.StoragePlugin.REMOVE_INSTANCE, id).unwrapOrNull()
        _onRemoveStorageEvent.emit(Unit)
        reload()
    }

    /**
     * A plugin backend registered a storage instance via
     * `ease.context.createStorage` (the non-OAuth create path, e.g. the
     * WebDAV plugin's `webdav:connect`). Reload + notify the create form.
     */
    suspend fun onPluginStorageCreated() {
        reload()
        _pluginConnectedEvent.emit(Unit)
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
