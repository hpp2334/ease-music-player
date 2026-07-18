package com.kutedev.easemusicplayer

import com.kutedev.easemusicplayer.di.appModule
import com.kutedev.easemusicplayer.platform.AppPaths
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.DesktopPermissionManager
import com.kutedev.easemusicplayer.singleton.PermissionManager
import com.kutedev.easemusicplayer.singleton.PlayerController
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.koin.core.context.startKoin
import org.koin.core.context.stopKoin
import org.koin.dsl.module
import uniffi.ease_client_backend.ArgAddMusicsToPlaylist
import uniffi.ease_client_backend.ArgCreatePlaylist
import uniffi.ease_client_backend.ArgUpsertStorage
import uniffi.ease_client_backend.ListStorageEntryChildrenResp
import uniffi.ease_client_backend.ctAddMusicsToPlaylist
import uniffi.ease_client_backend.ctCreatePlaylist
import uniffi.ease_client_backend.ctGetAssetStream
import uniffi.ease_client_backend.ctListStorage
import uniffi.ease_client_backend.ctListStorageEntryChildren
import uniffi.ease_client_backend.ctRemovePlaylist
import uniffi.ease_client_backend.ctRemoveStorage
import uniffi.ease_client_backend.ctUpsertStorage
import uniffi.ease_client_schema.DataSourceKey
import uniffi.ease_client_schema.PlaylistId
import uniffi.ease_client_schema.StorageEntryLoc
import uniffi.ease_client_schema.StorageId
import uniffi.ease_client_schema.StorageType
import uniffi.ease_client_backend.ToAddMusicEntry
import java.util.Locale
import kotlin.system.measureNanoTime

/**
 * Mirrors the StreamingHttpServer.handleMusic path: calls ctGetAssetStream
 * and drains the stream via AssetStream.next() until null.
 */
class WebdavStreamTest {

    private lateinit var bridge: Bridge
    private var playlistId: PlaylistId? = null
    private var storageId: StorageId? = null

    @Before
    fun setup() {
        Locale.setDefault(Locale.ENGLISH)
        val testDir = java.nio.file.Files.createTempDirectory("ease-stream-test").toFile()
        java.nio.file.Files.createDirectories(testDir.toPath().resolve("blobs"))

        startKoin {
            modules(
                appModule,
                module {
                    single {
                        AppPaths(
                            documentDir = testDir.absolutePath + "/",
                            cacheDir = System.getProperty("java.io.tmpdir") + "/ease-stream-test/",
                        )
                    }
                    single<PlayerController> { TestPlayerController(get()) }
                    single<PermissionManager> { DesktopPermissionManager() }
                }
            )
        }
        bridge = org.koin.core.context.GlobalContext.get().get<Bridge>()
        bridge.initialize()
    }

    @After
    fun tearDown() {
        try { bridge.destroy() } catch (_: Exception) {}
        stopKoin()
    }

    @Test
    fun streamWebdavMusicToCompletion() {
        runBlocking {
            // 1. Create playlist
            val createRet = bridge.run {
                ctCreatePlaylist(it, ArgCreatePlaylist(
                    title = "Stream Test", cover = null, entries = emptyList()
                ))
            }!!
            playlistId = createRet.id

            // 2. Add WebDAV storage
            bridge.run {
                ctUpsertStorage(it, ArgUpsertStorage(
                    id = null,
                    addr = "http://local.hpp2334.com:5000",
                    alias = "Test WebDAV",
                    username = "world",
                    password = "a123456",
                    isAnonymous = false,
                    typ = StorageType.WEBDAV,
                ))
            }
            val storages = bridge.run { ctListStorage(it) }!!
            storageId = storages.first { s -> s.alias == "Test WebDAV" }.id
            println("Storage ID: $storageId")

            // 3. List /data/musics, find melt memory.wav
            val musicDir = StorageEntryLoc(storageId!!, "/data/musics")
            val resp = bridge.run { ctListStorageEntryChildren(it, musicDir) }
            val entries = (resp as ListStorageEntryChildrenResp.Ok).v1
            val wavEntry = entries.find { it.name == "melt memory.wav" }
                ?: error("melt memory.wav not found; entries=${entries.map { it.name }}")
            println("Found: ${wavEntry.name} at ${wavEntry.path}")

            // 4. Add to playlist to get a MusicId
            val added = bridge.run {
                ctAddMusicsToPlaylist(it, ArgAddMusicsToPlaylist(
                    id = playlistId!!,
                    entries = listOf(ToAddMusicEntry(entry = wavEntry, name = "melt memory"))
                ))
            }!!
            val musicId = added.first { !it.existed }.id
            println("Music ID: $musicId")

            // 5. Drain the stream EXACTLY like StreamingHttpServer.handleMusic does
            val elapsedNs = measureNanoTime {
                val stream = bridge.run {
                    ctGetAssetStream(it, DataSourceKey.Music(musicId), 0uL)
                } ?: error("stream was null")

                val expectedSize = stream.size()
                println("Stream size: $expectedSize")
                var total = 0L
                var chunks = 0
                while (true) {
                    val chunk = stream.next() ?: break
                    chunks++
                    total += chunk.size.toLong()
                    if (chunks <= 3 || chunks % 50 == 0) {
                        println("chunk #$chunks: ${chunk.size} bytes (running total=$total)")
                    }
                }
                stream.close()
                println("DONE: $chunks chunks, $total bytes")
                println("Expected size: $expectedSize")
            }
            println("Elapsed: ${elapsedNs / 1_000_000} ms")

            // cleanup
            bridge.run { ctRemovePlaylist(it, playlistId!!) }
            storageId?.let { id -> bridge.run { ctRemoveStorage(it, id) } }
        }
    }
}
