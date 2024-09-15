package com.kutedev.easemusicplayer.core

import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import uniffi.ease_client.ArgInitializeApp
import uniffi.ease_client.IFlushSignal
import uniffi.ease_client.InvokeRet
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.bindFlushSignal
import uniffi.ease_client.flushSchedule
import uniffi.ease_client.initializeClient
import java.util.Timer
import kotlin.concurrent.schedule

typealias OnNotifyView = (view: RootViewModelState) -> Unit;

interface IOnNotifyView {
    fun onNotifyView(view: RootViewModelState);
}

class BitmapResources {
    private val store: HashMap<ULong, ImageBitmap> = HashMap()

    fun add(id: ULong, buf: ByteArray) {
        val bm = BitmapFactory.decodeByteArray(buf, 0, buf.size)
        val imageBm = bm.asImageBitmap()
        store[id] = imageBm
    }

    fun remove(id: ULong) {
        store.remove(id)
    }

    fun get(id: ULong): ImageBitmap? {
        return store[id]
    }
}

object Bridge {
    private class FlushSignalImpl : IFlushSignal {
        override fun flush() {
            Timer("Flush signal", false).schedule(0) {
                invoke {
                    flushSchedule()
                }
            }
        }
    }
    private val _store: HashSet<IOnNotifyView> = HashSet();
    private val _resources = BitmapResources()
    private val _player = MusicPlayer()

    fun getResources(): BitmapResources {
        return _resources
    }

    fun getPlayer(): MusicPlayer {
        return _player
    }

    fun initApp(context: android.content.Context) {
        bindFlushSignal(FlushSignalImpl())
        _player.install(context)

        invoke {
            initializeClient(
                ArgInitializeApp(
                    context.filesDir.absolutePath,
                    1u,
                    "/"
                ),
                _player
            )
        }
    }

    fun invoke(f: () -> InvokeRet) {
        val ret = f();
        val changedView = ret.view;
        if (changedView != null) {
            for (view in _store) {
                view.onNotifyView(changedView);
            }
        }

        val changedActions = ret.resources
        for (action in changedActions) {
            if (action.buf != null) {
                _resources.add(action.id, action.buf!!)
            } else {
                _resources.remove(action.id)
            }
        }
    }

    fun registerView(f: IOnNotifyView) {
        _store.add(f);
    }

    fun unregisterView(f: IOnNotifyView) {
        _store.remove(f);
    }
}