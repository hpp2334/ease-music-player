package com.kutedev.easemusicplayer.singleton

import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.ProgressiveMediaSource
import com.kutedev.easemusicplayer.core.BuildMediaContext
import com.kutedev.easemusicplayer.core.MusicPlayerDataSource
import com.kutedev.easemusicplayer.core.buildMediaItem
import com.kutedev.easemusicplayer.core.syncMetadataUtil
import kotlinx.collections.immutable.persistentListOf
import kotlinx.collections.immutable.toPersistentList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.time.debounce
import uniffi.ease_client_backend.AddedMusic
import uniffi.ease_client_backend.ArgCreatePlaylist
import uniffi.ease_client_backend.ArgReorderPlaylist
import uniffi.ease_client_backend.ArgUpdatePlaylist
import uniffi.ease_client_backend.PlaylistAbstract
import uniffi.ease_client_backend.ctCreatePlaylist
import uniffi.ease_client_backend.ctListPlaylist
import uniffi.ease_client_backend.ctRemovePlaylist
import uniffi.ease_client_backend.ctUpdatePlaylist
import uniffi.ease_client_backend.ctsGetMusicAbstract
import uniffi.ease_client_backend.ctsReorderPlaylist
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId
import java.time.Duration
import javax.inject.Inject
import javax.inject.Singleton


@Singleton
class PlaylistRepository @Inject constructor(
    private val bridge: Bridge,
    private val _scope: CoroutineScope
) {
    private val _requestSemaphore = Semaphore(4)
    private val _playlists = MutableStateFlow(persistentListOf<PlaylistAbstract>())
    private val _syncedTotalDuration = MutableSharedFlow<MusicId>()
    private val _debouncedReloadEvent = MutableSharedFlow<Unit>()

    val playlists = _playlists.asStateFlow()
    val syncedTotalDuration = _syncedTotalDuration.asSharedFlow()

    init {
        _scope.launch {
            _debouncedReloadEvent.debounce(Duration.ofMillis(500)).collect {
                reload()
            }
        }
    }

    fun createPlaylist(context: Context, arg: ArgCreatePlaylist) {
        _scope.launch {
            val created = bridge.run { ctCreatePlaylist(it, arg) }
            if ((created?.musicIds?.size ?: 0) > 0) {
                requestTotalDuration(context, created!!.musicIds)
            }
            reload()
        }
    }

    fun editPlaylist(arg: ArgUpdatePlaylist) {
        _scope.launch {
            bridge.run { ctUpdatePlaylist(it, arg) }
            reload()
        }
    }

    fun removePlaylist(id: PlaylistId) {
        _scope.launch {
            bridge.run { ctRemovePlaylist(it, id) }
            reload()
        }
    }

    fun requestTotalDuration(context: Context, added: List<AddedMusic>) {
        for (item in added) {
            if (!item.existed) {
                requestTotalDuration(context, item.id)
            }
        }
    }

    fun playlistMoveTo(fromIndex: Int, toIndex: Int) {
        val from = _playlists.value.getOrNull(fromIndex) ?: return

        _playlists.value = _playlists.value
            .removeAt(fromIndex)
            .add(toIndex, from)

        val a = _playlists.value.getOrNull(toIndex - 1)
        val b = _playlists.value.getOrNull(toIndex + 1)

        _scope.launch {
            bridge.runSync { ctsReorderPlaylist(it, ArgReorderPlaylist(
                id = from.meta.id,
                a = a?.meta?.id,
                b = b?.meta?.id))
            }
            scheduleReload()
        }
    }

    @OptIn(UnstableApi::class)
    private fun requestTotalDuration(context: Context, id: MusicId) {
        val musicAbstract = bridge.runSync { ctsGetMusicAbstract(it, id) } ?: return
        if (musicAbstract.meta.duration != null) {
            return
        }

        _scope.launch(Dispatchers.Main) {
            _requestSemaphore.acquire()

            try {
                val player = ExoPlayer.Builder(context)
                    .setMediaSourceFactory(ProgressiveMediaSource.Factory(DataSource.Factory { MusicPlayerDataSource(bridge, _scope) }) )
                    .build()
                player.addListener(object : Player.Listener {
                    override fun onPlaybackStateChanged(playbackState: Int) {
                        if (playbackState == Player.STATE_READY) {
                            syncMetadataUtil(
                                scope = _scope,
                                bridge = bridge,
                                player = player
                            ) {
                                _scope.launch {
                                    _syncedTotalDuration.emit(id)
                                    reload()
                                }
                            }
                            player.release()
                            _requestSemaphore.release()

                        }
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        player.release()
                        _requestSemaphore.release()
                        easeError("request total duration failed: $error")
                    }
                })

                val mediaItem = buildMediaItem(BuildMediaContext(
                    bridge = bridge,
                    scope = _scope,
                ), musicAbstract)
                player.setMediaItem(mediaItem)
                player.prepare()
            } catch (error: Exception) {
                easeError("request total duration failed: $error")
            }
        }
    }

    fun scheduleReload() {
        _scope.launch {
            _debouncedReloadEvent.emit(Unit)
        }
    }

    suspend fun reload() {
        _playlists.value = bridge.run { ctListPlaylist(it).toPersistentList() } ?: persistentListOf()
    }
}