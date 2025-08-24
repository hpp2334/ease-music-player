package com.kutedev.easemusicplayer.core

import android.net.Uri
import androidx.annotation.OptIn
import androidx.media3.common.C.LENGTH_UNSET
import androidx.media3.common.C.RESULT_END_OF_INPUT
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.TransferListener
import com.kutedev.easemusicplayer.singleton.Bridge
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import uniffi.ease_client_backend.AssetStream
import uniffi.ease_client_backend.ctGetAssetStream
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_schema.DataSourceKey
import uniffi.ease_client_schema.MusicId
import java.io.PipedInputStream
import java.io.PipedOutputStream
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

@OptIn(UnstableApi::class)
class MusicPlayerDataSource(
    private val bridge: Bridge,
    private val scope: CoroutineScope
) : DataSource {
    private var _loadJob: Job? = null
    private var _currentUri: Uri? = null
    private var _inputStream: PipedInputStream? = null

    override fun addTransferListener(transferListener: TransferListener) {
        // noop
    }

    override fun open(dataSpec: DataSpec): Long {
        reset()

        val raw = dataSpec.uri.getQueryParameter("music")
        val musicId = raw?.toLong()?.let { MusicId(it) }

        if (musicId == null) {
            throw RuntimeException("music id $raw not found")
        }

        _currentUri = dataSpec.uri

        val assetStream = runBlocking {
            bridge.run { ctGetAssetStream(it, DataSourceKey.Music(musicId)) }
        }
        if (assetStream == null) {
            throw RuntimeException("music $raw not found")
        }

        val input = PipedInputStream()
        val output = PipedOutputStream(input)
        _inputStream = input
        _loadJob = scope.launch {
            while (true) {
                try {
                    val b = assetStream.next()
                    if (b == null) {
                        break
                    }
                    output.write(b)
                } catch (e: Exception) {
                    easeError("load chunk failed, $e")
                    break
                }
            }
            output.close()
        }

        val fileSize = assetStream.size()
        if (fileSize != null) {
            return fileSize.toLong()
        }
        return LENGTH_UNSET.toLong()
    }

    override fun getUri(): Uri? {
        return _currentUri
    }

    override fun close() {
        reset()
    }

    override fun read(
        buffer: ByteArray,
        offset: Int,
        length: Int
    ): Int {
        val stream = _inputStream ?: return RESULT_END_OF_INPUT
        val read = stream.read(buffer, offset, length)
        return if (read == -1) RESULT_END_OF_INPUT else read
    }

    private fun reset() {
        _loadJob?.cancel()
        _loadJob = null
        _currentUri = null
        _inputStream = null
    }
}