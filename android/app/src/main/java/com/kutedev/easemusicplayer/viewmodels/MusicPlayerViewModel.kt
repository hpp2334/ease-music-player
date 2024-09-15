package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.media3.common.Player
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.core.currentPositionFlow
import kotlinx.coroutines.launch
import uniffi.ease_client.setCurrentMusicPositionForPlayerInternal
import uniffi.ease_client.updateCurrentMusicPlayingForPlayerInternal

class MusicPlayerViewModel : ViewModel() {
    private var _playerListener: Player.Listener? = null

    fun initialize() {
        val player = Bridge.getPlayer().getInternal()

        _playerListener = object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                Bridge.invoke {
                    updateCurrentMusicPlayingForPlayerInternal(isPlaying)
                }
            }
        }
        player.addListener(_playerListener!!)

        viewModelScope.launch {
            player.currentPosition
            player.currentPositionFlow().collect {_ ->
                Bridge.invoke {
                    setCurrentMusicPositionForPlayerInternal(player.currentPosition.toULong())
                }
            }
        }
    }

    fun destroy() {
        val player = Bridge.getPlayer().getInternal()
        if (_playerListener != null) {
            player.removeListener(_playerListener!!)
        }
    }
}