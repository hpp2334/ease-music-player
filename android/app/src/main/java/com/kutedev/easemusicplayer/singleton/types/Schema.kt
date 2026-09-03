package com.kutedev.easemusicplayer.singleton.types

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.KSerializer
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer

// ============================================================================
// IDs — mirror Rust newtypes (i64). Rust's `define_id!` macro uses
// `#[serde(transparent)]`, so IDs serialize as bare JSON numbers:
//   MusicId(42)  ↔  42
//
// Inline value classes give us JVM primitive erasure + Kotlin call-site
// ergonomics (no allocation per instance). kotlinx.serialization's inline
// class serializer produces a bare JsonPrimitive when used at the top
// level — matching Rust's transparent shape exactly.
// ============================================================================

@Serializable
@JvmInline
value class MusicId(val value: Long)

@Serializable
@JvmInline
value class PlaylistId(val value: Long)

@Serializable
@JvmInline
value class StorageId(val value: Long)

// Plugin storage-contribution ids (mirror the Rust `PluginId` /
// `PluginStorageId` newtypes). Rust uses `#[serde(transparent)]`, so these
// serialize as bare JSON strings.
@Serializable
@JvmInline
value class PluginId(val id: String)

@Serializable
@JvmInline
value class PluginStorageId(val id: String)

// ============================================================================
// Enums
// ============================================================================

@Serializable
enum class PlayMode { SINGLE, SINGLE_LOOP, LIST, LIST_LOOP }

// `StorageType` is gone — replaced by `StorageHandle`, which carries the kind
// AND the kind-specific id. Surfaced on `Storage.handle`.

@Serializable
enum class StorageEntryType { FOLDER, MUSIC, IMAGE, LYRIC, OTHER }

@Serializable
enum class LyricLoadState { LOADING, MISSING, FAILED, LOADED }

@Serializable
enum class CreatePlaylistMode { FULL, EMPTY }

@Serializable
enum class CurrentStorageStateType { LOADING, OK, NEED_PERMISSION, AUTHENTICATION_FAILED, TIMEOUT, UNKNOWN_ERROR }

// ============================================================================
// Schema records
// ============================================================================

@Serializable
data class StorageEntryLoc(
    val storageId: StorageId,
    val path: String,
)

// Parametric storage descriptor — the `kind` tag matches the Rust enum's
// `#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]`; the
// per-variant fields are camelCase.
@Serializable
sealed class StorageHandle {
    @Serializable
    @SerialName("LOCAL")
    data object Local : StorageHandle()

    @Serializable
    @SerialName("PLUGIN")
    data class Plugin(
        @SerialName("pluginId") val pluginId: PluginId,
        @SerialName("pluginStorageId") val pluginStorageId: PluginStorageId,
    ) : StorageHandle()
}

@Serializable
sealed class DataSourceKey {
    @Serializable
    @SerialName("MUSIC")
    data class Music(val id: MusicId) : DataSourceKey()

    @Serializable
    @SerialName("COVER")
    data class Cover(val id: MusicId) : DataSourceKey()

    @Serializable
    @SerialName("ANY_ENTRY")
    data class AnyEntry(val entry: StorageEntryLoc) : DataSourceKey()
}

// ============================================================================
// Domain records (mirror ease-client-backend/src/objects/*.rs)
// ============================================================================

@Serializable
data class MusicMeta(
    val id: MusicId,
    val title: String,
    val duration: Long? = null,
    val order: List<Long> = emptyList(),
)

@Serializable
data class MusicAbstract(
    val meta: MusicMeta,
    val cover: DataSourceKey? = null,
)

@Serializable
data class MusicLyric(
    val loc: StorageEntryLoc,
    val data: Lyrics,
    val loadedState: LyricLoadState,
)

@Serializable
data class Music(
    val meta: MusicMeta,
    val loc: StorageEntryLoc,
    val cover: DataSourceKey? = null,
    val lyric: MusicLyric? = null,
)

@Serializable
data class LrcMetadata(
    val artist: String = "",
    val album: String = "",
    val title: String = "",
    val lyricist: String = "",
    val author: String = "",
    val length: String = "",
    val offset: String = "",
)

@Serializable
data class LyricLine(
    val duration: Long,
    val text: String,
)

@Serializable
data class Lyrics(
    val metdata: LrcMetadata = LrcMetadata(),
    val lines: List<LyricLine> = emptyList(),
)

@Serializable
data class PlaylistMeta(
    val id: PlaylistId,
    val title: String,
    val cover: StorageEntryLoc? = null,
    val showCover: DataSourceKey? = null,
    val createdTime: Long,
    val order: List<Long> = emptyList(),
)

@Serializable
data class PlaylistAbstract(
    val meta: PlaylistMeta,
    val musicCount: ULong,
    val duration: Long? = null,
)

@Serializable
data class Playlist(
    @SerialName("abstr") val abstr: PlaylistAbstract,
    val musics: List<MusicAbstract>,
)

@Serializable
data class Storage(
    val id: StorageId,
    val handle: StorageHandle,
    val alias: String,
    val musicCount: ULong,
)

@Serializable
data class StorageEntry(
    val storageId: StorageId,
    val name: String,
    val path: String,
    val size: ULong? = null,
    val isDir: Boolean,
)

@Serializable
sealed class ListStorageEntryChildrenResp {
    @Serializable
    @SerialName("OK")
    data class Ok(val data: List<StorageEntry>) : ListStorageEntryChildrenResp()

    @Serializable
    @SerialName("AUTHENTICATION_FAILED")
    data object AuthenticationFailed : ListStorageEntryChildrenResp()

    @Serializable
    @SerialName("TIMEOUT")
    data object Timeout : ListStorageEntryChildrenResp()

    @Serializable
    @SerialName("UNKNOWN")
    data object Unknown : ListStorageEntryChildrenResp()
}

@Serializable
data class ListLogFile(
    val name: String,
    val path: String,
)

@Serializable
data class ListLogFiles(
    val files: List<ListLogFile>,
)

@Serializable
data class AddedMusic(
    val id: MusicId,
    val existed: Boolean,
)

@Serializable
data class RetCreatePlaylist(
    val id: PlaylistId,
    val musicIds: List<AddedMusic>,
)

@Serializable
data class MetadataRecord(
    val format: AudioFormatRecord,
    val durationMs: ULong? = null,
    val tags: List<TagRecord> = emptyList(),
    val hasCover: Boolean = false,
)

@Serializable
data class AudioFormatRecord(
    val channels: UInt,
    val sampleRate: UInt,
)

@Serializable
data class TagRecord(
    val key: String,
    val value: String,
)

// ============================================================================
// Argument structs
// ============================================================================

@Serializable
data class ArgInitializeApp(
    val appDocumentDir: String,
    val appCacheDir: String,
    val storagePath: String,
)

@Serializable
data class ArgOauthUrl(
    val pluginId: String,
    val oauthId: String,
)

@Serializable
data class ArgOauthExchange(
    val pluginId: String,
    val oauthId: String,
    val code: String,
)

@Serializable
data class PluginOauthUrl(val url: String)

@Serializable
data class PluginOauthExchangeResult(val storageId: StorageId)

@Serializable
data class ArgUpdatePlaylist(
    val id: PlaylistId,
    val title: String,
    val cover: StorageEntryLoc? = null,
)

@Serializable
data class ArgCreatePlaylist(
    val title: String,
    val cover: StorageEntryLoc? = null,
    val entries: List<ToAddMusicEntry>,
)

@Serializable
data class ArgAddMusicsToPlaylist(
    val id: PlaylistId,
    val entries: List<ToAddMusicEntry>,
)

@Serializable
data class ToAddMusicEntry(
    val entry: StorageEntry,
    val name: String,
)

@Serializable
data class ArgRemoveMusicFromPlaylist(
    val playlistId: PlaylistId,
    val musicId: MusicId,
)

@Serializable
data class ArgUpdateMusicLyric(
    val id: MusicId,
    val lyricLoc: StorageEntryLoc? = null,
)

@Serializable
data class ArgReorderPlaylist(
    val id: PlaylistId,
    val a: PlaylistId? = null,
    val b: PlaylistId? = null,
)

@Serializable
data class ArgReorderMusic(
    val playlistId: PlaylistId,
    val id: MusicId,
    val a: MusicId? = null,
    val b: MusicId? = null,
)

// ============================================================================
// Argument structs — bridge-only (no Rust-side counterpart; the dispatcher
// unpacks these from inline `struct Args { ... }` shapes).
// ============================================================================

@Serializable
data class ArgPluginEvent(
    val pluginId: String,
    val type: String,
    val payload: JsonElement,
)

@Serializable
data class ArgUpdateMusicDuration(
    val id: MusicId,
    val duration: Long,
)

// ============================================================================
// Bridge envelope
// ============================================================================

@Serializable
data class BridgeResponse(
    val success: Boolean,
    val payload: JsonElement? = null,
    val buffers: List<ByteArray>? = null,
    val errorCode: String? = null,
    val errorDetail: JsonElement? = null,
)

class BridgeException(val code: String, val detail: JsonElement?) :
    RuntimeException("[$code] ${detail ?: ""}")

// ============================================================================
// Player pollState batched response
// — moved to cantode's Kotlin half (com.kutedev.cantode): the engine owns
// its own wire (FfiPollSnapshot / PlayerState) and the app no longer
// defines player-poll types here.
// ============================================================================
