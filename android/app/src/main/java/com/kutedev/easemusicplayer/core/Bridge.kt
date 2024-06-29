package com.kutedev.easemusicplayer.core

import uniffi.ease_client.ArgInitializeApp
import uniffi.ease_client.InvokeRet
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.initializeClient

typealias OnNotifyView = (view: RootViewModelState) -> Unit;

interface IOnNotifyView {
    fun onNotifyView(view: RootViewModelState);
}

object Bridge {
    private val store: HashSet<IOnNotifyView> = HashSet();

    fun initApp(context: android.content.Context) {
        invoke {
            initializeClient(
                ArgInitializeApp(
                    context.filesDir.absolutePath,
                    1u,
                    "/"
                )
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