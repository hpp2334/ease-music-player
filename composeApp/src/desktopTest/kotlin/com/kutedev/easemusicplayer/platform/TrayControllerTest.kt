package com.kutedev.easemusicplayer.platform

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import java.awt.MenuItem
import java.awt.SystemTray
import java.awt.event.ActionEvent
import javax.swing.SwingUtilities

class TrayControllerTest {

    private lateinit var scope: CoroutineScope
    private val playing = MutableStateFlow(false)
    private var showCount = 0
    private var toggleCount = 0
    private var quitCount = 0
    private lateinit var controller: TrayController

    @Before
    fun setup() {
        scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        showCount = 0
        toggleCount = 0
        quitCount = 0
        playing.value = false
        controller = TrayController(
            playingFlow = playing,
            scope = scope,
            onShow = { showCount++ },
            onPlayPauseToggle = { toggleCount++ },
            onQuit = { quitCount++ }
        )
    }

    @After
    fun tearDown() {
        controller.remove()
        scope.cancel()
    }

    @Test
    fun icon_resource_is_on_classpath() {
        val url = Thread.currentThread().contextClassLoader?.getResource("ic_launcher.png")
        assertNotNull("ic_launcher.png must be on the desktop runtime classpath", url)
    }

    @Test
    fun isSupported_matches_system_tray() {
        assertEquals(SystemTray.isSupported(), controller.isSupported)
    }

    @Test
    fun install_registers_tray_icon_when_supported() {
        assumeTrue("SystemTray is not supported on this host", controller.isSupported)
        assertTrue("install() should return true when supported", controller.install())
        assertTrue(controller.isInstalled)
        val icon = controller.trayIcon
        assertNotNull("trayIcon should be set after install", icon)
        assertTrue(
            "tray icon should be registered in the system tray",
            SystemTray.getSystemTray().trayIcons.any { it === icon }
        )
    }

    @Test
    fun remove_clears_isInstalled() {
        assumeTrue(controller.isSupported)
        assertTrue(controller.install())
        controller.remove()
        assertFalse("isInstalled must be false after remove", controller.isInstalled)
        assertTrue(
            "tray icon must be removed from the system tray",
            SystemTray.getSystemTray().trayIcons.none { it === controller.trayIcon }
        )
    }

    @Test
    fun double_install_is_rejected() {
        assumeTrue(controller.isSupported)
        assertTrue(controller.install())
        val second = controller.install()
        assertFalse("a second install() while one is active must not add a second icon", second)
        assertEquals(
            "exactly one tray icon should be present",
            1,
            SystemTray.getSystemTray().trayIcons.count { it === controller.trayIcon }
        )
    }

    @Test
    fun show_menu_item_fires_onShow() {
        assumeTrue(controller.isSupported)
        assertTrue(controller.install())
        fireAction(findItem("Show"))
        assertEquals(1, showCount)
    }

    @Test
    fun quit_menu_item_fires_onQuit() {
        assumeTrue(controller.isSupported)
        assertTrue(controller.install())
        fireAction(findItem("Quit"))
        assertEquals(1, quitCount)
    }

    @Test
    fun play_pause_menu_item_fires_toggle() {
        assumeTrue(controller.isSupported)
        assertTrue(controller.install())
        val item = findPlayPauseItem()
        fireAction(item)
        fireAction(item)
        assertEquals(2, toggleCount)
    }

    @Test
    fun play_pause_label_tracks_playing_state() {
        assumeTrue(controller.isSupported)
        assertTrue(controller.install())

        val item = findPlayPauseItem()
        waitForLabel(item, "Play")
        assertEquals("not playing -> label should be Play", "Play", labelOnEdt(item))

        playing.value = true
        waitForLabel(item, "Pause")
        assertEquals("playing -> label should be Pause", "Pause", labelOnEdt(item))

        playing.value = false
        waitForLabel(item, "Play")
        assertEquals("paused again -> label should be Play", "Play", labelOnEdt(item))
    }

    @Test
    fun remove_when_not_installed_is_safe() {
        // Should not throw even if install() was never called.
        controller.remove()
        assertFalse(controller.isInstalled)
    }

    private fun findItem(label: String): MenuItem {
        val popup = controller.trayIcon!!.popupMenu
        return itemsOf(popup).first { it.label == label }
    }

    private fun findPlayPauseItem(): MenuItem {
        val popup = controller.trayIcon!!.popupMenu
        return itemsOf(popup).first { it.label == "Play" || it.label == "Pause" }
    }

    private fun itemsOf(menu: java.awt.Menu): List<MenuItem> =
        (0 until menu.itemCount).map { menu.getItem(it) }

    private fun fireAction(item: MenuItem) {
        val evt = ActionEvent(item, ActionEvent.ACTION_PERFORMED, item.label)
        item.actionListeners.forEach { it.actionPerformed(evt) }
    }

    private fun labelOnEdt(item: MenuItem): String {
        val out = arrayOf("")
        SwingUtilities.invokeAndWait { out[0] = item.label }
        return out[0]
    }

    private fun waitForLabel(item: MenuItem, expected: String, timeoutMs: Long = 3000) {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            if (labelOnEdt(item) == expected) return
            Thread.sleep(25)
        }
    }
}
