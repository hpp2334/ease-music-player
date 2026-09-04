@file:Suppress("unused")

package com.kutedev.easemusicplayer.singleton

import com.kutedev.easemusicplayer.singleton.types.AddedMusic
import com.kutedev.easemusicplayer.singleton.types.ArgAddMusicsToPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgCreatePlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgPluginEvent
import com.kutedev.easemusicplayer.singleton.types.ArgPluginBaseUrl
import com.kutedev.easemusicplayer.singleton.types.ArgPluginId
import com.kutedev.easemusicplayer.singleton.types.ArgPluginInstallFromRegistry
import com.kutedev.easemusicplayer.singleton.types.ArgPluginInstallZipPath
import com.kutedev.easemusicplayer.singleton.types.ArgPluginSetEnable
import com.kutedev.easemusicplayer.singleton.types.ArgPluginSourceAddCustom
import com.kutedev.easemusicplayer.singleton.types.PluginListResult
import com.kutedev.easemusicplayer.singleton.types.PluginMutationResult
import com.kutedev.easemusicplayer.singleton.types.PluginSourcesResult
import com.kutedev.easemusicplayer.singleton.types.RegistryEntriesResult
import com.kutedev.easemusicplayer.singleton.types.ArgReorderMusic
import com.kutedev.easemusicplayer.singleton.types.ArgReorderPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgRemoveMusicFromPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgUpdateMusicDuration
import com.kutedev.easemusicplayer.singleton.types.ArgUpdateMusicLyric
import com.kutedev.easemusicplayer.singleton.types.ArgUpdatePlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgOauthExchange
import com.kutedev.easemusicplayer.singleton.types.ArgOauthUrl
import com.kutedev.easemusicplayer.singleton.types.ListLogFiles
import com.kutedev.easemusicplayer.singleton.types.ListStorageEntryChildrenResp
import com.kutedev.easemusicplayer.singleton.types.MusicAbstract
import com.kutedev.easemusicplayer.singleton.types.MusicId
import com.kutedev.easemusicplayer.singleton.types.MusicLyric
import com.kutedev.easemusicplayer.singleton.types.PlaylistAbstract
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import com.kutedev.easemusicplayer.singleton.types.PlayMode
import com.kutedev.easemusicplayer.singleton.types.PluginOauthExchangeResult
import com.kutedev.easemusicplayer.singleton.types.PluginOauthUrl
import com.kutedev.easemusicplayer.singleton.types.RetCreatePlaylist
import com.kutedev.easemusicplayer.singleton.types.StorageEntryLoc
import com.kutedev.easemusicplayer.singleton.types.StorageId
// Types below shadow the namespace objects nested in [BridgeMethods]; import
// them under `T*` aliases so the spec type parameters resolve to data classes
// (not namespace objects) inside each `object` body.
import com.kutedev.easemusicplayer.singleton.types.Music as TMusic
import com.kutedev.easemusicplayer.singleton.types.Playlist as TPlaylist
import com.kutedev.easemusicplayer.singleton.types.Storage as TStorage

/**
 * Typed catalog of every bridge method that has a regular shape:
 * single typed arg (or no arg) + single typed return, with the handle
 * determined by namespace.
 *
 * Methods NOT in this catalog (called via [Bridge.callRaw] instead):
 *  - `backend.create/init/deinit/log` — lifecycle, internal to [Bridge].
 *  - `asset.get` — returns binary buffer, not JSON payload.
 *  - `player.contextNew`, `player.new` — return raw `{handle: N}` for
 *    engine setup.
 *  - `player.loadMusic` — cross-handle arg (`backendHandle` + `musicId`).
 *  - `player.probeDurationMs` — cross-handle arg (`contextHandle` +
 *    `backendHandle` + `musicId`); used by [PlaylistRepository] to
 *    pre-fill music duration on import without playback.
 *  - `music.updateCover` — buffer input.
 *  - `debug.triggerError`, `debug.triggerPanic` — diagnostic; caller
 *    wants the error envelope, not success.
 *
 * Each entry is a [BridgeSpec] value carrying the method's wire name,
 * arg serializer, return serializer, and handle kind. Type parameters
 * pin the call-site contract: `bridge.call(BridgeMethods.Playlist.GET, id)`
 * cannot mismatch arg or ret types at compile time.
 *
 * Usage:
 * ```
 * val playlist: Playlist? = bridge.call(BridgeMethods.Playlist.GET, id)
 *     .unwrapOrNull()?.payload
 * val list: List<PlaylistAbstract>? = bridge.call(BridgeMethods.Playlist.LIST)
 *     .unwrapOrNull()?.payload
 * ```
 *
 * Inside each `object` body, the namespace name (e.g. `Music`) refers to
 * the namespace object itself; data classes that share the name are
 * imported under `T*` aliases to disambiguate (e.g. `TMusic`).
 */
object BridgeMethods {

    /** `storage.*` — registry CRUD + entry browsing (kind-agnostic). */
    object Storage {
        val LIST = bridgeSpecNoArg<List<TStorage>>("storage.list")
        val REMOVE = bridgeSpecArg<StorageId, Unit>("storage.remove")
        val LIST_ENTRY_CHILDREN =
            bridgeSpecArg<StorageEntryLoc, ListStorageEntryChildrenResp>("storage.listEntryChildren")
    }

    /** `oauth.*` — OAuth flow for JS plugin providers (e.g. OneDrive). The
     *  JS ops are contract literals (`oauth:url` / `oauth:exchange`); the
     *  plugin's identity (`pluginId`, from the instance data slot) and the
     *  host-minted flow token (`oauthId`, from `ease.oauth.new()`) ride the
     *  args. Business data (the alias) stays inside the plugin. */
    object Oauth {
        val URL = bridgeSpecArg<ArgOauthUrl, PluginOauthUrl>("oauth.url")
        val EXCHANGE = bridgeSpecArg<ArgOauthExchange, PluginOauthExchangeResult>("oauth.exchange")
    }

    /** `storage_plugin.*` — storage-instance lifecycle for JS plugin storage
     *  providers. Non-OAuth providers (WebDAV) create their instances from
     *  their own setup view via a backend RPC + `ease.context.createStorage`
     *  instead. */
    object StoragePlugin {
        val REMOVE_INSTANCE =
            bridgeSpecArg<StorageId, Unit>("storage_plugin.remove_instance")
    }

    /** `playlist.*` — playlist CRUD + music membership + reorder. */
    object Playlist {
        val LIST = bridgeSpecNoArg<List<PlaylistAbstract>>("playlist.list")
        val GET = bridgeSpecArg<PlaylistId, TPlaylist?>("playlist.get")
        val CREATE = bridgeSpecArg<ArgCreatePlaylist, RetCreatePlaylist>("playlist.create")
        val UPDATE = bridgeSpecArg<ArgUpdatePlaylist, Unit>("playlist.update")
        val REMOVE = bridgeSpecArg<PlaylistId, Unit>("playlist.remove")
        val ADD_MUSICS = bridgeSpecArg<ArgAddMusicsToPlaylist, List<AddedMusic>>("playlist.addMusics")
        val REMOVE_MUSIC =
            bridgeSpecArg<ArgRemoveMusicFromPlaylist, Unit>("playlist.removeMusic")
        val REORDER = bridgeSpecArg<ArgReorderPlaylist, Unit>("playlist.reorder")
        val REORDER_MUSIC = bridgeSpecArg<ArgReorderMusic, Unit>("playlist.reorderMusic")
    }

    /** `music.*` — music metadata access + updates. */
    object Music {
        val GET = bridgeSpecArg<MusicId, TMusic?>("music.get")
        val GET_ABSTRACT = bridgeSpecArg<MusicId, MusicAbstract?>("music.getAbstract")

        /**
         * Network-bound lyric fetch — the follow-up to [GET]. [GET] is
         * DB-only and returns the lyric as a `LOADING` placeholder; this
         * loads + parses the bytes over the storage seam so the caller can
         * patch the result in without gating the track switch on it.
         */
        val LOAD_LYRIC = bridgeSpecArg<MusicId, MusicLyric?>("music.loadLyric")
        val UPDATE_LYRIC = bridgeSpecArg<ArgUpdateMusicLyric, Unit>("music.updateLyric")
        val UPDATE_DURATION = bridgeSpecArg<ArgUpdateMusicDuration, Unit>("music.updateDuration")
    }

    /** `preference.*` — user preferences (currently just PlayMode). */
    object Preference {
        val GET_PLAY_MODE = bridgeSpecNoArg<PlayMode>("preference.getPlayMode")
        val SAVE_PLAY_MODE = bridgeSpecArg<PlayMode, Unit>("preference.savePlayMode")
    }

    /**
     * `player.*` — nothing typed remains: transport ops (play/pause/stop/
     * seek/setVolume) and the state observables moved to cantode's own
     * Kotlin half (`com.kutedev.cantode.Cantode`, via cantode's JNI
     * bridge under the same handle id). The biz-owned player methods —
     * `player.contextNew` / `player.new` (creation) and
     * `player.loadMusic` (source construction + metadata writeback) —
     * don't fit the typed catalog (raw `{handle}` / cross-handle args)
     * and are called via [com.kutedev.easemusicplayer.singleton.Bridge.callRaw].
     */

    /** `plugin.*` — plugin event dispatch + the Rust-side install layer. */
    object Plugin {
        val EVENT = bridgeSpecArg<ArgPluginEvent, Unit>("plugin.event")
        val LIST = bridgeSpecNoArg<PluginListResult>("plugin.list")
        val INSTALL_ZIP_PATH = bridgeSpecArg<ArgPluginInstallZipPath, PluginMutationResult>("plugin.installZipPath")
        val INSTALL_FROM_REGISTRY =
            bridgeSpecArg<ArgPluginInstallFromRegistry, PluginMutationResult>("plugin.installFromRegistry")
        val SET_ENABLE = bridgeSpecArg<ArgPluginSetEnable, PluginMutationResult>("plugin.setEnable")
        val UNINSTALL = bridgeSpecArg<ArgPluginId, PluginMutationResult>("plugin.uninstall")
        val BOOTSTRAP = bridgeSpecNoArg<PluginMutationResult>("plugin.bootstrap")
        val REGISTRY_FETCH = bridgeSpecArg<ArgPluginBaseUrl, RegistryEntriesResult>("plugin.registryFetch")
        val REGISTRY_CACHED = bridgeSpecArg<ArgPluginBaseUrl, RegistryEntriesResult>("plugin.registryCached")
        val SOURCES_LIST = bridgeSpecNoArg<PluginSourcesResult>("plugin.sourcesList")
        val SOURCE_REMEMBER = bridgeSpecArg<ArgPluginBaseUrl, Unit>("plugin.sourceRemember")
        val SOURCE_ADD_CUSTOM = bridgeSpecArg<ArgPluginSourceAddCustom, RegistryEntriesResult>("plugin.sourceAddCustom")
        val SOURCE_REMOVE_CUSTOM = bridgeSpecArg<ArgPluginBaseUrl, Unit>("plugin.sourceRemoveCustom")
    }

    /** `debug.*` — diagnostic helpers. */
    object Debug {
        val LIST_LOG_FILES = bridgeSpecNoArg<ListLogFiles>("debug.listLogFiles")
    }
}
