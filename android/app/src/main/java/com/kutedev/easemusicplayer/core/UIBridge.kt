package com.kutedev.easemusicplayer.core

import AsyncRuntimeAdapter
import android.Manifest.permission.READ_EXTERNAL_STORAGE
import android.Manifest.permission.READ_MEDIA_AUDIO
import android.annotation.SuppressLint
import android.content.ComponentName
import android.content.Intent
import android.content.Intent.FLAG_ACTIVITY_NEW_TASK
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import androidx.activity.result.ActivityResultLauncher
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import androidx.navigation.NavHostController
import com.google.common.util.concurrent.MoreExecutors
import com.kutedev.easemusicplayer.utils.nextTickOnMain
import uniffi.ease_client.MainAction
import uniffi.ease_client_android.IPermissionServiceForeign
import uniffi.ease_client_android.IRouterServiceForeign
import uniffi.ease_client_android.IToastServiceForeign
import uniffi.ease_client_android.IViewStateServiceForeign
import uniffi.ease_client.MainBodyWidget
import uniffi.ease_client.MusicControlWidget
import uniffi.ease_client.MusicDetailWidget
import uniffi.ease_client.MusicLyricWidget
import uniffi.ease_client.PlaylistCreateWidget
import uniffi.ease_client.PlaylistDetailWidget
import uniffi.ease_client.PlaylistEditWidget
import uniffi.ease_client.PlaylistListWidget
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.RouterAction
import uniffi.ease_client.RoutesKey
import uniffi.ease_client.StorageImportWidget
import uniffi.ease_client.StorageListWidget
import uniffi.ease_client.StorageUpsertWidget
import uniffi.ease_client.TimeToPauseWidget
import uniffi.ease_client.ViewAction
import uniffi.ease_client.Widget
import uniffi.ease_client.WidgetAction
import uniffi.ease_client.WidgetActionType
import uniffi.ease_client_android.apiBuildClient
import uniffi.ease_client_android.apiDestroyClient
import uniffi.ease_client_android.apiEmitViewAction
import uniffi.ease_client_android.apiLoadAsset
import uniffi.ease_client_android.apiStartClient
import uniffi.ease_client_shared.DataSourceKey


interface IOnNotifyView {
    fun onNotifyView(v: RootViewModelState);
}

private class ViewStates : IViewStateServiceForeign {
    private val _store: HashSet<IOnNotifyView> = HashSet();

    override fun handleNotify(v: RootViewModelState) {
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

private class ToastService : IToastServiceForeign {
    private var context: android.content.Context? = null

    fun setContext(context: android.content.Context) {
        this.context = context
    }

    override fun error(msg: String) {
        val context = this.context!!;
        android.widget.Toast.makeText(context, msg, android.widget.Toast.LENGTH_SHORT).show()
    }
}

class RouterService : IRouterServiceForeign {
    private var _navigatorController: NavHostController? = null;

    fun install(controller: NavHostController) {
        _navigatorController = controller
    }
    fun destroy() {
        _navigatorController = null
    }

    override fun naviagate(key: RoutesKey) {
        if (_navigatorController != null) {
            _navigatorController!!.navigate(key.toString())
        }
    }

    override fun pop() {
        if (_navigatorController != null) {
            _navigatorController!!.navigateUp()
        }
    }
}

private class PermissionService : IPermissionServiceForeign {
    private var context: android.content.Context? = null
    private var requestPermissionLauncher: ActivityResultLauncher<String>? = null

    fun setContext(context: android.content.Context, requestPermissionLauncher: ActivityResultLauncher<String>) {
        this.context = context
        this.requestPermissionLauncher = requestPermissionLauncher
    }

    override fun openUrl(url: String) {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
        intent.flags = FLAG_ACTIVITY_NEW_TASK
        context?.startActivity(intent)
    }

    override fun haveStoragePermission(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context?.checkSelfPermission(READ_MEDIA_AUDIO) == PackageManager.PERMISSION_GRANTED
        } else {
            context?.checkSelfPermission(READ_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED
        }
    }

    override fun requestStoragePermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            this.requestPermissionLauncher?.launch(READ_MEDIA_AUDIO)
        } else {
            this.requestPermissionLauncher?.launch(READ_EXTERNAL_STORAGE)
        }
    }
}


class DataSourceKeyH(key: DataSourceKey) {
    private val _key = key;

    fun value(): DataSourceKey {
        return this._key
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is DataSourceKeyH) return false

        if (this._key is DataSourceKey.Music && other._key is DataSourceKey.Music) {
            return this._key.id == other._key.id
        }
        if (this._key is DataSourceKey.Cover && other._key is DataSourceKey.Cover) {
            return this._key.id == other._key.id
        }
        if (this._key is DataSourceKey.AnyEntry && other._key is DataSourceKey.AnyEntry) {
            return this._key.entry.storageId == other._key.entry.storageId && this._key.entry.path == other._key.entry.path;
        }
        return false;
    }

    override fun hashCode(): Int {
        return _key.hashCode()
    }
}

class BitmapDataSources {
    private val _map = HashMap<DataSourceKeyH, ImageBitmap?>()

    fun get(key: DataSourceKeyH): ImageBitmap? {
        return this._map[key]
    }

    suspend fun load(key: DataSourceKeyH): ImageBitmap? {
        val cached = this._map[key]
        if (cached != null) {
            return cached
        }

        val data = apiLoadAsset(key.value())
        val decoded = data?.let {
            val bitmap = android.graphics.BitmapFactory.decodeByteArray(it, 0, it.size)
            bitmap?.asImageBitmap()
        }
        this._map[key] = decoded
        return decoded
    }
}

class UIBridge {
    private var _handle: ULong = 0uL

    @SuppressLint("StaticFieldLeak")
    private val _viewStates = ViewStates()
    @SuppressLint("StaticFieldLeak")
    private val _toastService = ToastService()
    val routerInternal = RouterService()
    @SuppressLint("StaticFieldLeak")
    private val _permissionService = PermissionService()
    private var _playerController: MediaController? = null
    private var _executingAction = false
    val bitmapDataSources = BitmapDataSources()

    fun onBackendConnected() {
        apiStartClient(this._handle)
    }

    fun onActivityCreate(context: android.content.Context,
                         requestPermissionLauncher: ActivityResultLauncher<String>) {
        _toastService.setContext(context)
        _permissionService.setContext(context, requestPermissionLauncher)

        val factory = MediaController.Builder(
            context,
            SessionToken(context, ComponentName(context, PlaybackService::class.java))
        ).buildAsync()
        factory.addListener(
            {
                _playerController = factory.let {
                    if (it.isDone)
                        it.get()
                    else
                        null
                }
            },
            MoreExecutors.directExecutor()
        )

        this._handle = apiBuildClient(
            _permissionService,
            routerInternal,
            _toastService,
            _viewStates,
            AsyncRuntimeAdapter()
        )
    }

    fun onActivityStart() {
        dispatchAction(ViewAction.Main(MainAction.ON_MAIN_WIN_SHOWN))
    }

    fun onActivityStop() {
        dispatchAction(ViewAction.Main(MainAction.ON_MAIN_WIN_HIDDEN))
    }

    fun onActivityDestroy() {
        routerInternal.destroy()
        apiDestroyClient(this._handle)
        _playerController?.release()
        _playerController = null
    }
    private fun dispatchClick(widget: Widget) {
        dispatchAction(ViewAction.Widget(WidgetAction(widget, WidgetActionType.Click)))
    }

    private fun dispatchChangeText(widget: Widget, text: String) {
        dispatchAction(ViewAction.Widget(WidgetAction(widget, WidgetActionType.ChangeText(text))))
    }

    fun schedule(block: () -> Unit) {
        if (!this._executingAction) {
            block()
        } else {
            nextTickOnMain {
                block()
            }
        }
    }

    fun dispatchAction(action: ViewAction) {
        this._executingAction = true
        apiEmitViewAction(this._handle, action)
        this._executingAction = false
    }

    fun popRoute() {
        dispatchAction(ViewAction.Router(RouterAction.POP))
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
