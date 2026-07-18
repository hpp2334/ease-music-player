package com.kutedev.easemusicplayer.lifecycle

import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

class AppLifecycle(
    private val scope: CoroutineScope,
    private val playerRepository: PlayerRepository,
    private val storageRepository: StorageRepository,
    private val playlistRepository: PlaylistRepository,
) {
    fun onStartup() {
        scope.launch {
            playerRepository.reload()
            storageRepository.reload()
            playlistRepository.reload()
        }
    }
}
