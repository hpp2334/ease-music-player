package com.kutedev.easemusicplayer

import com.kutedev.easemusicplayer.di.appModule
import com.kutedev.easemusicplayer.platform.AppPaths
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.DesktopPermissionManager
import com.kutedev.easemusicplayer.singleton.PermissionManager
import com.kutedev.easemusicplayer.singleton.PlayerController
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.viewmodel.compose.LocalViewModelStoreOwner
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.koin.core.context.startKoin
import org.koin.core.context.stopKoin
import org.koin.dsl.module
import uniffi.ease_client_backend.ArgAddMusicsToPlaylist
import uniffi.ease_client_backend.ArgUpsertStorage
import uniffi.ease_client_backend.StorageEntry
import uniffi.ease_client_backend.StorageEntryType
import uniffi.ease_client_backend.ToAddMusicEntry
import uniffi.ease_client_backend.ctAddMusicsToPlaylist
import uniffi.ease_client_backend.ctCreatePlaylist
import uniffi.ease_client_backend.ctGetMusic
import uniffi.ease_client_backend.ctGetPlaylist
import uniffi.ease_client_backend.ctListPlaylist
import uniffi.ease_client_backend.ctListStorage
import uniffi.ease_client_backend.ctListStorageEntryChildren
import uniffi.ease_client_backend.ctRemovePlaylist
import uniffi.ease_client_backend.ctRemoveStorage
import uniffi.ease_client_backend.ctUpsertStorage
import uniffi.ease_client_schema.PlaylistId
import uniffi.ease_client_schema.StorageId
import uniffi.ease_client_schema.StorageType
import java.awt.image.BufferedImage
import java.io.File
import javax.imageio.ImageIO

class UserFlowTest {

    @get:Rule
    val composeRule = createComposeRule()

    private lateinit var bridge: Bridge
    private lateinit var testController: TestPlayerController
    private lateinit var playerRepo: PlayerRepository
    private var playlistId: PlaylistId? = null
    private var storageId: StorageId? = null

    @Before
    fun setup() {
        java.util.Locale.setDefault(java.util.Locale.ENGLISH)
        val testDir = java.nio.file.Files.createTempDirectory("ease-test-db").toFile()
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
                    single<PlayerController> {
                        testController = TestPlayerController(get())
                        testController
                    }
                    single<PermissionManager> { DesktopPermissionManager() }
                }
            )
        }
        bridge = org.koin.core.context.GlobalContext.get().get<Bridge>()
        playerRepo = org.koin.core.context.GlobalContext.get().get()
        bridge.initialize()
    }

    @After
    fun tearDown() {
        try { bridge.destroy() } catch (_: Exception) {}
        stopKoin()
    }

    private fun saveScreenshot(name: String) {
        val bitmap = composeRule.onNodeWithTag("screen_root").captureToImage()
        val pixelMap = bitmap.toPixelMap()
        val w = bitmap.width
        val h = bitmap.height
        val image = BufferedImage(w, h, BufferedImage.TYPE_INT_RGB)
        for (y in 0 until h) {
            for (x in 0 until w) {
                val c = pixelMap[x, y]
                val rgb = ((c.red * 255).toInt() shl 16) or
                          ((c.green * 255).toInt() shl 8) or
                          (c.blue * 255).toInt()
                image.setRGB(x, y, rgb)
            }
        }
        val dir = File(System.getProperty("java.io.tmpdir"), "ease-user-flow")
        dir.mkdirs()
        val f = File(dir, name)
        ImageIO.write(image, "PNG", f)
        println("Saved: ${f.absolutePath} (${f.length()} bytes)")
    }

    private fun startApp() {
        composeRule.mainClock.autoAdvance = true
        composeRule.setContent {
            val lifecycleOwner = remember {
                object : LifecycleOwner {
                    override val lifecycle: Lifecycle = LifecycleRegistry(this).apply {
                        handleLifecycleEvent(Lifecycle.Event.ON_CREATE)
                        handleLifecycleEvent(Lifecycle.Event.ON_START)
                        handleLifecycleEvent(Lifecycle.Event.ON_RESUME)
                    }
                }
            }
            val viewModelStoreOwner = remember {
                object : ViewModelStoreOwner {
                    override val viewModelStore: ViewModelStore = ViewModelStore()
                }
            }
            CompositionLocalProvider(
                LocalLifecycleOwner provides lifecycleOwner,
                LocalViewModelStoreOwner provides viewModelStoreOwner,
            ) {
                Box(Modifier.fillMaxSize().testTag("screen_root")) {
                    Root()
                }
            }
        }
        composeRule.waitForIdle()
        Thread.sleep(2000)
        composeRule.waitForIdle()
    }

    @Test
    fun fullUserFlow() {
        startApp()
        saveScreenshot("uf-01-initial.png")

        // === STEP 1: Create playlist via UI ===
        println("=== STEP 1: Create playlist ===")
        composeRule.onNodeWithText("Empty Playlists. Tap to add one.").performClick()
        composeRule.waitForIdle(); Thread.sleep(500)
        saveScreenshot("uf-02-dialog.png")

        composeRule.onNodeWithText("EMPTY").performClick()
        composeRule.waitForIdle(); Thread.sleep(500)
        saveScreenshot("uf-03-empty-tab.png")

        composeRule.onNode(hasSetTextAction()).performTextInput("My Test Playlist")
        composeRule.waitForIdle(); Thread.sleep(500)
        saveScreenshot("uf-04-name-entered.png")

        composeRule.mainClock.autoAdvance = false
        composeRule.onAllNodesWithText("OK")[0].performClick()
        Thread.sleep(3000)
        composeRule.mainClock.autoAdvance = true
        composeRule.waitForIdle()
        saveScreenshot("uf-05-playlist-created.png")

        // === STEP 2: Import music via backend (simulating WebDAV import) ===
        println("=== STEP 2: Import music from WebDAV ===")
        composeRule.mainClock.autoAdvance = false
        runBlocking {
            val playlists = bridge.run { ctListPlaylist(it) }
            playlistId = playlists!!.first { p -> p.meta.title == "My Test Playlist" }.meta.id

            bridge.run {
                ctUpsertStorage(it, ArgUpsertStorage(
                    id = null, addr = "http://local.hpp2334.com:5000",
                    alias = "Test WebDAV", username = "world", password = "a123456",
                    isAnonymous = false, typ = StorageType.WEBDAV,
                ))
            }
            val storages = bridge.run { ctListStorage(it) }
            storageId = storages!!.first { s -> s.alias == "Test WebDAV" }.id
            println("Storage ID: $storageId")

            val musicDir = uniffi.ease_client_schema.StorageEntryLoc(storageId!!, "/data/musics")
            val resp = bridge.run { ctListStorageEntryChildren(it, musicDir) }
            val entries = (resp as uniffi.ease_client_backend.ListStorageEntryChildrenResp.Ok).v1
            val wavEntry = entries.find { it.name == "melt memory.wav" }!!
            println("Found: ${wavEntry.name} at ${wavEntry.path}")

            val toAdd = listOf(ToAddMusicEntry(
                entry = wavEntry,
                name = "melt memory",
            ))
            val added = bridge.run {
                ctAddMusicsToPlaylist(it, ArgAddMusicsToPlaylist(
                    id = playlistId!!, entries = toAdd
                ))
            }
            println("Added music: $added")
        }
        composeRule.mainClock.autoAdvance = true
        composeRule.waitForIdle(); Thread.sleep(2000); composeRule.waitForIdle()
        saveScreenshot("uf-06-music-imported.png")

        // === STEP 3: Test playback ===
        println("=== STEP 3: Test playback ===")
        composeRule.mainClock.autoAdvance = false
        runBlocking {
            val playlist = bridge.run {
                ctGetPlaylist(it, playlistId!!)
            }!!
            val musicAbstract = playlist.musics.first()
            val musicId = musicAbstract.meta.id
            println("Playing music: ${musicAbstract.meta.title} (id=$musicId)")

            val music = bridge.run { ctGetMusic(it, musicId) }!!
            testController.play(musicId, playlistId!!)
            playerRepo.setCurrent(music, playlist)
        }
        composeRule.mainClock.autoAdvance = true
        composeRule.waitForIdle(); Thread.sleep(1000); composeRule.waitForIdle()
        saveScreenshot("uf-07-playing.png")
        println("Play calls: ${testController.playCallCount}, isPlaying: ${testController.isPlaying}")

        // === STEP 4: Pause ===
        println("=== STEP 4: Pause ===")
        testController.pause()
        composeRule.waitForIdle(); Thread.sleep(500); composeRule.waitForIdle()
        saveScreenshot("uf-08-paused.png")
        println("Pause calls: ${testController.pauseCallCount}, isPlaying: ${testController.isPlaying}")

        // === STEP 5: Stop + Remove playlist ===
        println("=== STEP 5: Stop and remove playlist ===")
        testController.stop()
        composeRule.waitForIdle(); Thread.sleep(500); composeRule.waitForIdle()

        composeRule.mainClock.autoAdvance = false
        runBlocking {
            bridge.run { ctRemovePlaylist(it, playlistId!!) }
            if (storageId != null) {
                bridge.run { ctRemoveStorage(it, storageId!!) }
            }
        }
        composeRule.mainClock.autoAdvance = true
        composeRule.waitForIdle(); Thread.sleep(2000); composeRule.waitForIdle()
        saveScreenshot("uf-09-removed.png")

        println("=== ALL STEPS COMPLETE ===")
        println("Play calls: ${testController.playCallCount}")
        println("Pause calls: ${testController.pauseCallCount}")
    }
}
