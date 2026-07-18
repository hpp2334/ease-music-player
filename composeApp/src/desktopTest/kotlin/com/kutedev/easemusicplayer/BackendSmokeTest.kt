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
import uniffi.ease_client_backend.ArgCreatePlaylist
import uniffi.ease_client_backend.ArgUpsertStorage
import uniffi.ease_client_backend.ctCreatePlaylist
import uniffi.ease_client_backend.ctListPlaylist
import uniffi.ease_client_backend.ctListStorage
import uniffi.ease_client_backend.ctRemovePlaylist
import uniffi.ease_client_backend.ctRemoveStorage
import uniffi.ease_client_backend.ctUpsertStorage
import uniffi.ease_client_schema.StorageType
import java.util.Locale

class BackendSmokeTest {

    private lateinit var bridge: Bridge

    @Before
    fun setup() {
        Locale.setDefault(Locale.ENGLISH)
        val testDir = java.nio.file.Files.createTempDirectory("ease-test-db").toFile()
        println("Test DB dir: ${testDir.absolutePath}")
        java.nio.file.Files.createDirectories(testDir.toPath().resolve("blobs"))

        startKoin {
            modules(
                appModule,
                module {
                    single {
                        AppPaths(
                            documentDir = testDir.absolutePath + "/",
                            cacheDir = System.getProperty("java.io.tmpdir") + "/ease-music-player-test/",
                        )
                    }
                    single<PlayerController> { TestPlayerController(get()) }
                    single<PermissionManager> { DesktopPermissionManager() }
                }
            )
        }
        bridge = org.koin.core.context.GlobalContext.get().get<Bridge>()
        println("Initializing bridge...")
        bridge.initialize()
        println("Bridge initialized.")
    }

    @After
    fun tearDown() {
        try { bridge.destroy() } catch (_: Exception) {}
        stopKoin()
    }

    @Test
    fun testCreateEmptyPlaylist() {
        println("=== Testing ctCreatePlaylist with empty entries ===")
        runBlocking {
            val result = bridge.run {
                ctCreatePlaylist(it, ArgCreatePlaylist(
                    title = "Backend Test Playlist",
                    cover = null,
                    entries = emptyList()
                ))
            }
            println("Created playlist: $result")

            println("=== Listing playlists ===")
            val playlists = bridge.run { ctListPlaylist(it) }
            playlists?.forEach { p ->
                println("  Playlist: ${p.meta.title} (id=${p.meta.id})")
            }

            println("=== Cleaning up ===")
            bridge.run {
                ctListPlaylist(it)?.forEach { p ->
                    ctRemovePlaylist(it, p.meta.id)
                }
            }
            println("=== Done ===")
        }
    }

    @Test
    fun testAddWebDAVStorage() {
        println("=== Testing ctUpsertStorage with WebDAV ===")
        runBlocking {
            bridge.run {
                ctUpsertStorage(it, ArgUpsertStorage(
                    id = null,
                    addr = "http://localhost:5000",
                    alias = "Test WebDAV",
                    username = "world",
                    password = "a123456",
                    isAnonymous = false,
                    typ = StorageType.WEBDAV,
                ))
            }
            println("Storage upserted")

            println("=== Listing storages ===")
            val storages = bridge.run { ctListStorage(it) }
            storages?.forEach { s ->
                println("  Storage: ${s.alias} (id=${s.id}, typ=${s.typ})")
            }

            println("=== Cleaning up ===")
            bridge.run {
                ctListStorage(it)?.forEach { s ->
                    if (s.alias.contains("Test")) ctRemoveStorage(it, s.id)
                }
            }
            println("=== Done ===")
        }
    }
}
