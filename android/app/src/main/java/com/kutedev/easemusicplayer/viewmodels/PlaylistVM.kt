package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.repositories.PlaylistRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.MusicAbstract
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.PlaylistAbstract
import uniffi.ease_client_schema.PlaylistId
import uniffi.ease_client_backend.PlaylistMeta
import java.time.Duration
import javax.inject.Inject



@HiltViewModel
class PlaylistVM @Inject constructor(
    private val playlistRepository: PlaylistRepository
) : ViewModel() {
    private val _removeModalOpen = MutableStateFlow(false)
    private val _editModalOpen = MutableStateFlow(false)
    private val _playlist = MutableStateFlow(Playlist(
        abstr = PlaylistAbstract(
            meta = PlaylistMeta(
                id = PlaylistId(0),
                title = "",
                cover = null,
                showCover = null,
                createdTime = Duration.ofMillis(0L)
            ),
            musicCount = 0uL,
            duration = null
        ),
        musics = emptyList()
    ))
    val editModalOpen = _editModalOpen.asStateFlow()
    val removeModalOpen = _removeModalOpen.asStateFlow()
    val playlist = _playlist.asStateFlow()

    fun remove() {}

    fun removeMusic(id: MusicId) {}

    fun openRemoveModal() {}

    fun closeRemoveModal() {}

    fun openEditModal() {}

    fun closeEditModal() {}
}

private fun _durationStr(duration: Duration?): String {
    if (duration != null) {
        val all = duration.toMillis()
        val h = all / 1000 / 60 / 60
        val m = all / 1000 / 60 % 60
        val s = all / 1000 % 60
        return "${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}"
    } else {
        return ""
    }
}

fun PlaylistAbstract.durationStr(): String {
    return _durationStr(duration)
}

fun MusicAbstract.durationStr(): String {
    return _durationStr(meta.duration)
}