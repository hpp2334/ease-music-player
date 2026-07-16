package com.kutedev.easemusicplayer.audio

import com.kutedev.easemusicplayer.singleton.Bridge
import com.sun.net.httpserver.HttpExchange
import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.runBlocking
import uniffi.ease_client_backend.ctGetAssetStream
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_schema.DataSourceKey
import uniffi.ease_client_schema.MusicId
import java.io.IOException
import java.net.InetSocketAddress
import java.util.concurrent.Executors

class StreamingHttpServer(
    private val bridge: Bridge,
    private val scope: CoroutineScope
) {
    private var server: HttpServer? = null
    private var _port: Int = 0

    val baseUrl: String
        get() = "http://127.0.0.1:$_port"

    val port: Int
        get() = _port

    fun start() {
        val s = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        s.createContext("/music/", ::handleMusic)
        s.executor = Executors.newCachedThreadPool()
        s.start()
        server = s
        _port = s.address.port
    }

    fun stop() {
        server?.stop(0)
        (server?.executor as? java.util.concurrent.ExecutorService)?.shutdownNow()
        server = null
    }

    private fun handleMusic(exchange: HttpExchange) {
        val path = exchange.requestURI.path
        val idStr = path.removePrefix("/music/")
        val musicId = idStr.toLongOrNull()

        if (musicId == null) {
            exchange.sendResponseHeaders(404, -1)
            exchange.close()
            return
        }

        val rangeHeader = exchange.requestHeaders.getFirst("Range")
        val rangeStart = parseRangeStart(rangeHeader)

        runBlocking {
            val assetStream = bridge.run {
                ctGetAssetStream(it, DataSourceKey.Music(MusicId(musicId)), rangeStart.toULong())
            }

            if (assetStream == null) {
                exchange.sendResponseHeaders(404, -1)
                exchange.close()
                return@runBlocking
            }

            val totalSize = assetStream.size()?.toLong()

            exchange.responseHeaders.set("Accept-Ranges", "bytes")
            exchange.responseHeaders.set("Content-Type", "application/octet-stream")

            if (rangeHeader != null && totalSize != null) {
                val contentLength = totalSize - rangeStart
                val endByte = totalSize - 1
                exchange.responseHeaders.set(
                    "Content-Range",
                    "bytes $rangeStart-$endByte/$totalSize"
                )
                exchange.sendResponseHeaders(206, contentLength)
            } else {
                exchange.sendResponseHeaders(200, totalSize ?: -1L)
            }

            val output = exchange.responseBody
            try {
                while (true) {
                    val chunk = assetStream.next() ?: break
                    output.write(chunk)
                    output.flush()
                }
            } catch (_: IOException) {
            } catch (e: Exception) {
                easeError("streaming error: $e")
            } finally {
                try { output.close() } catch (_: Exception) {}
                assetStream.close()
                exchange.close()
            }
        }
    }

    private fun parseRangeStart(rangeHeader: String?): Long {
        if (rangeHeader == null) return 0L
        val match = Regex("bytes=(\\d+)-").find(rangeHeader) ?: return 0L
        return match.groupValues[1].toLongOrNull() ?: 0L
    }
}
