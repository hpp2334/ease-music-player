package com.kutedev.easemusicplayer.platform

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import java.awt.AWTException
import java.awt.Color
import java.awt.Dimension
import java.awt.EventQueue
import java.awt.Graphics2D
import java.awt.MenuItem
import java.awt.PopupMenu
import java.awt.RenderingHints
import java.awt.SystemTray
import java.awt.TrayIcon
import java.awt.event.MouseAdapter
import java.awt.event.MouseEvent
import java.awt.image.BufferedImage
import javax.imageio.ImageIO

class TrayController(
    private val playingFlow: StateFlow<Boolean>,
    private val scope: CoroutineScope,
    private val onShow: () -> Unit,
    private val onPlayPauseToggle: () -> Unit,
    private val onQuit: () -> Unit
) {
    internal var trayIcon: TrayIcon? = null
    private var playPauseItem: MenuItem? = null
    private var labelJob: Job? = null

    val isSupported: Boolean get() = SystemTray.isSupported()
    val isInstalled: Boolean get() = trayIcon != null

    fun install(): Boolean {
        if (trayIcon != null) return false
        if (!isSupported) return false
        val tray = SystemTray.getSystemTray()
        val image = loadIcon(tray.trayIconSize)

        val popup = PopupMenu().apply {
            add(MenuItem("Show").apply { addActionListener { onShow() } })
            val pp = MenuItem(playPauseLabel(playingFlow.value)).apply {
                addActionListener { onPlayPauseToggle() }
            }
            playPauseItem = pp
            add(pp)
            addSeparator()
            add(MenuItem("Quit").apply { addActionListener { onQuit() } })
        }

        val icon = TrayIcon(image, "Ease Music Player", popup).apply {
            isImageAutoSize = true
            addMouseListener(object : MouseAdapter() {
                override fun mouseClicked(e: MouseEvent) {
                    if (e.button == MouseEvent.BUTTON1) onShow()
                }
            })
        }

        return try {
            tray.add(icon)
            trayIcon = icon
            labelJob = scope.launch(Dispatchers.Default) {
                playingFlow.collect { playing ->
                    val label = playPauseLabel(playing)
                    EventQueue.invokeLater { playPauseItem?.label = label }
                }
            }
            true
        } catch (_: AWTException) {
            trayIcon = null
            false
        }
    }

    fun remove() {
        labelJob?.cancel()
        labelJob = null
        trayIcon?.let { icon ->
            runCatching { SystemTray.getSystemTray().remove(icon) }
        }
        trayIcon = null
    }

    private fun playPauseLabel(playing: Boolean) = if (playing) "Pause" else "Play"

    private fun loadIcon(size: Dimension): BufferedImage {
        val raw = runCatching {
            ImageIO.read(Thread.currentThread().contextClassLoader?.getResourceAsStream("ic_launcher.png"))
        }.getOrNull() ?: return fallbackIcon(size)

        val target = if (size.width > 0 && size.height > 0) {
            maxOf(size.width, size.height)
        } else 32

        val scaled = BufferedImage(target, target, BufferedImage.TYPE_INT_ARGB)
        val g = scaled.createGraphics() as Graphics2D
        g.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_BILINEAR)
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON)
        g.drawImage(raw, 0, 0, target, target, null)
        g.dispose()
        return scaled
    }

    private fun fallbackIcon(size: Dimension): BufferedImage {
        val dim = if (size.width > 0) maxOf(size.width, size.height) else 32
        val img = BufferedImage(dim, dim, BufferedImage.TYPE_INT_ARGB)
        val g = img.createGraphics() as Graphics2D
        g.color = Color(0x3D, 0xDC, 0x84)
        g.fillRoundRect(0, 0, dim, dim, dim / 5, dim / 5)
        g.dispose()
        return img
    }
}
