package com.kutedev.easemusicplayer.core

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
    private val store: HashSet<IOnNotifyView> = HashSet();

    fun initApp(context: android.content.Context) {
        bindFlushSignal(FlushSignalImpl())

        invoke {
            initializeClient(
                ArgInitializeApp(
                    context.filesDir.absolutePath,
                    1u,
                    "/"
                ),
                MusicPlayer()
            )
        }
    }

    fun invoke(f: () -> InvokeRet) {
        val ret = f();
        val state = ret.view;
        if (state != null) {
            for (view in store) {
                view.onNotifyView(state);
            }
        }
    }

    fun registerView(f: IOnNotifyView) {
        store.add(f);
    }

    fun unregisterView(f: IOnNotifyView) {
        store.remove(f);
    }
}