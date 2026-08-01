@file:Suppress("unused")

package com.kutedev.easemusicplayer.singleton

import com.kutedev.easemusicplayer.singleton.types.AddedMusic
import com.kutedev.easemusicplayer.singleton.types.ArgAddMusicsToPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgCreatePlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgPluginKvAppend
import com.kutedev.easemusicplayer.singleton.types.ArgReorderMusic
import com.kutedev.easemusicplayer.singleton.types.ArgReorderPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgRemoveMusicFromPlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgUpdateMusicDuration
import com.kutedev.easemusicplayer.singleton.types.ArgUpdateMusicLyric
import com.kutedev.easemusicplayer.singleton.types.ArgUpdatePlaylist
import com.kutedev.easemusicplayer.singleton.types.ArgUpsertWebdavStorage
import com.kutedev.easemusicplayer.singleton.types.ListLogFiles
import com.kutedev.easemusicplayer.singleton.types.ListStorageEntryChildrenResp
import com.kutedev.easemusicplayer.singleton.types.MusicAbstract
import com.kutedev.easemusicplayer.singleton.types.MusicId
import com.kutedev.easemusicplayer.singleton.types.PlayerPollState
import com.kutedev.easemusicplayer.singleton.types.PlaylistAbstract
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import com.kutedev.easemusicplayer.singleton.types.PlayMode
import com.kutedev.easemusicplayer.singleton.types.RetCreatePlaylist
import com.kutedev.easemusicplayer.singleton.types.StorageConnectionTestResult
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

    /** `storage_webdav.*` — WebDAV-only create/update + connection test. */
    object StorageWebdav {
        val UPSERT = bridgeSpecArg<ArgUpsertWebdavStorage, Unit>("storage_webdav.upsert")
        val TEST = bridgeSpecArg<ArgUpsertWebdavStorage, StorageConnectionTestResult>("storage_webdav.test")
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
        val UPDATE_LYRIC = bridgeSpecArg<ArgUpdateMusicLyric, Unit>("music.updateLyric")
        val UPDATE_DURATION = bridgeSpecArg<ArgUpdateMusicDuration, Unit>("music.updateDuration")
    }

    /** `preference.*` — user preferences (currently just PlayMode). */
    object Preference {
        val GET_PLAY_MODE = bridgeSpecNoArg<PlayMode>("preference.getPlayMode")
        val SAVE_PLAY_MODE = bridgeSpecArg<PlayMode, Unit>("preference.savePlayMode")
    }

    /**
     * `player.*` — cantode transport ops. All methods resolve to the
     * player handle (not the backend handle).
     *
     * Single-value args (SEEK, SET_VOLUME) are sent bare — Rust reads
     * them via `let pos_ms: u64 = serde_json::from_value(req.args)?`.
     */
    object Player {
        val PLAY = bridgeSpecNoArg<Unit>("player.play", HandleKind.PLAYER)
        val PAUSE = bridgeSpecNoArg<Unit>("player.pause", HandleKind.PLAYER)
        val STOP = bridgeSpecNoArg<Unit>("player.stop", HandleKind.PLAYER)
        val SEEK = bridgeSpecArg<Long, Unit>("player.seek", HandleKind.PLAYER)
        val SET_VOLUME = bridgeSpecArg<Float, Unit>("player.setVolume", HandleKind.PLAYER)
        val POLL_STATE = bridgeSpecNoArg<PlayerPollState>("player.pollState", HandleKind.PLAYER)
    }

    /** `plugin.*` — plugin KV store (playcount etc.). */
    object Plugin {
        val KV_MULTI_APPEND = bridgeSpecArg<ArgPluginKvAppend, Unit>("plugin.kvMultiAppend")
    }

    /** `debug.*` — diagnostic helpers. */
    object Debug {
        val LIST_LOG_FILES = bridgeSpecNoArg<ListLogFiles>("debug.listLogFiles")
    }
}
