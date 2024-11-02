package com.kutedev.easemusicplayer.core

import android.annotation.SuppressLint
import uniffi.ease_client.IFlushNotifier
import uniffi.ease_client.IToastService
import uniffi.ease_client.IViewStateService
import uniffi.ease_client.MainBodyWidget
import uniffi.ease_client.MusicControlWidget
import uniffi.ease_client.MusicDetailWidget
import uniffi.ease_client.MusicLyricWidget
import uniffi.ease_client.PlaylistCreateWidget
import uniffi.ease_client.PlaylistDetailWidget
import uniffi.ease_client.PlaylistEditWidget
import uniffi.ease_client.PlaylistListWidget
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.StorageImportWidget
import uniffi.ease_client.StorageListWidget
import uniffi.ease_client.StorageUpsertWidget
import uniffi.ease_client.TimeToPauseWidget
import uniffi.ease_client.ViewAction
import uniffi.ease_client.Widget
import uniffi.ease_client.WidgetAction
import uniffi.ease_client.WidgetActionType
import uniffi.ease_client.apiBuildClient
import uniffi.ease_client.apiEmitViewAction
import uniffi.ease_client.apiFlushSpawned
import uniffi.ease_client.apiStartClient
import uniffi.ease_client_shared.ArgInitializeApp
import java.util.Timer
import kotlin.concurrent.schedule

typealias OnNotifyView = (view: RootViewModelState) -> Unit;

interface IOnNotifyView {
    fun onNotifyView(v: RootViewModelState);
}

private class FlushNotifier : IFlushNotifier {
    private var _isNotified = false;

    override fun notify() {
        if (!_isNotified) {
            _isNotified = true;
            Timer().schedule(0) {
                _isNotified = false
                apiFlushSpawned()
            }
        }
    }
}

private class ViewStates : IViewStateService {
    private val _store: HashSet<IOnNotifyView> = HashSet();

    override fun notify(v: RootViewModelState) {
        for (view in this._store) {
            view.onNotifyView(v)
        }
    }

    fun registerView(f: IOnNotifyView) {
        _store.add(f);
    }

    fun unregisterView(f: IOnNotifyView) {
        _store.remove(f);
    }
}

private class ToastService : IToastService {
    private var context: android.content.Context? = null

    fun setContext(context: android.content.Context) {
        this.context = context
    }

    override fun error(msg: String) {
        val context = this.context!!;
        android.widget.Toast.makeText(context, msg, android.widget.Toast.LENGTH_SHORT).show()
    }
}

object Bridge {
    private val _player = MusicPlayer()
    private val _viewStates = ViewStates()
    private val _flushNotifier = FlushNotifier()
    @SuppressLint("StaticFieldLeak")
    private val _toastService = ToastService()
    private const val SCHEMA_VERSION = 1u
    private const val STORAGE_PATH = "/"

    fun getPlayer(): MusicPlayer {
        return _player
    }

    fun initApp(context: android.content.Context) {
        _player.install(context)
        _toastService.setContext(context)

        apiBuildClient(
            _player,
            _toastService,
            _viewStates,
            _flushNotifier
        );
        apiStartClient(ArgInitializeApp(
            context.filesDir.absolutePath,
            SCHEMA_VERSION,
            STORAGE_PATH
        ))
    }

    fun dispatchClick(widget: Widget) {
        apiEmitViewAction(ViewAction.Widget(WidgetAction(widget, WidgetActionType.Click)))
    }

    fun dispatchChangeText(widget: Widget, text: String) {
        apiEmitViewAction(ViewAction.Widget(WidgetAction(widget, WidgetActionType.ChangeText(text))))
    }

    fun dispatchAction(action: ViewAction) {
        apiEmitViewAction(action)
    }

    fun dispatchClick(widget: MainBodyWidget) {
        dispatchClick(Widget.MainBody(widget))
    }

    fun dispatchChangeText(widget: MainBodyWidget, text: String) {
        dispatchChangeText(Widget.MainBody(widget), text)
    }

    fun dispatchClick(widget: MusicControlWidget) {
        dispatchClick(Widget.MusicControl(widget))
    }

    fun dispatchChangeText(widget: MusicControlWidget, text: String) {
        dispatchChangeText(Widget.MusicControl(widget), text)
    }

    fun dispatchClick(widget: MusicLyricWidget) {
        dispatchClick(Widget.MusicLyric(widget))
    }

    fun dispatchChangeText(widget: MusicLyricWidget, text: String) {
        dispatchChangeText(Widget.MusicLyric(widget), text)
    }

    fun dispatchClick(widget: MusicDetailWidget) {
        dispatchClick(Widget.MusicDetail(widget))
    }

    fun dispatchChangeText(widget: MusicDetailWidget, text: String) {
        dispatchChangeText(Widget.MusicDetail(widget), text)
    }

    fun dispatchClick(widget: TimeToPauseWidget) {
        dispatchClick(Widget.TimeToPause(widget))
    }

    fun dispatchChangeText(widget: TimeToPauseWidget, text: String) {
        dispatchChangeText(Widget.TimeToPause(widget), text)
    }

    fun dispatchClick(widget: PlaylistDetailWidget) {
        dispatchClick(Widget.PlaylistDetail(widget))
    }

    fun dispatchChangeText(widget: PlaylistDetailWidget, text: String) {
        dispatchChangeText(Widget.PlaylistDetail(widget), text)
    }

    fun dispatchClick(widget: PlaylistEditWidget) {
        dispatchClick(Widget.PlaylistEdit(widget))
    }

    fun dispatchChangeText(widget: PlaylistEditWidget, text: String) {
        dispatchChangeText(Widget.PlaylistEdit(widget), text)
    }

    fun dispatchClick(widget: PlaylistCreateWidget) {
        dispatchClick(Widget.PlaylistCreate(widget))
    }

    fun dispatchChangeText(widget: PlaylistCreateWidget, text: String) {
        dispatchChangeText(Widget.PlaylistCreate(widget), text)
    }

    fun dispatchClick(widget: PlaylistListWidget) {
        dispatchClick(Widget.PlaylistList(widget))
    }

    fun dispatchChangeText(widget: PlaylistListWidget, text: String) {
        dispatchChangeText(Widget.PlaylistList(widget), text)
    }

    fun dispatchClick(widget: StorageImportWidget) {
        dispatchClick(Widget.StorageImport(widget))
    }

    fun dispatchChangeText(widget: StorageImportWidget, text: String) {
        dispatchChangeText(Widget.StorageImport(widget), text)
    }

    fun dispatchClick(widget: StorageListWidget) {
        dispatchClick(Widget.StorageList(widget))
    }

    fun dispatchChangeText(widget: StorageListWidget, text: String) {
        dispatchChangeText(Widget.StorageList(widget), text)
    }

    fun dispatchClick(widget: StorageUpsertWidget) {
        dispatchClick(Widget.StorageUpsert(widget))
    }

    fun dispatchChangeText(widget: StorageUpsertWidget, text: String) {
        dispatchChangeText(Widget.StorageUpsert(widget), text)
    }

    fun registerView(f: IOnNotifyView) {
        _viewStates.registerView(f)
    }

    fun unregisterView(f: IOnNotifyView) {
        _viewStates.unregisterView(f)
    }
}
