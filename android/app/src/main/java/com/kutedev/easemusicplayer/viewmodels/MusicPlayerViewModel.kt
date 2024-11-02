package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.media3.common.Player
import com.kutedev.easemusicplayer.core.Bridge
import uniffi.ease_client.PlayerEvent
import uniffi.ease_client.ViewAction

class MusicPlayerViewModel : ViewModel() {
    private var _playerListener: Player.Listener? = null

    fun initialize() {
        val player = Bridge.getPlayer().getInternal()

        _playerListener = object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                if (isPlaying) {
                    Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Play));
                } else {
                    Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Pause));
                }
            }
        }
        player.addListener(_playerListener!!)
    }

    fun destroy() {
        val player = Bridge.getPlayer().getInternal()
        if (_playerListener != null) {
            player.removeListener(_playerListener!!)
        }
    }
}