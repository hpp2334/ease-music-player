import com.kutedev.easemusicplayer.utils.nextTickOnMain
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import uniffi.ease_client_android.IAsyncAdapterForeign
import uniffi.ease_client_android.apiFlushBackendSpawnedLocal
import uniffi.ease_client_android.apiFlushSpawnedLocals
import java.util.Timer
import java.util.TimerTask


class AsyncRuntimeAdapter : IAsyncAdapterForeign {
    override fun onSpawnLocals(appId: ULong?) {
        nextTickOnMain {
            if (appId != null) {
                apiFlushSpawnedLocals(appId)
            } else {
                apiFlushBackendSpawnedLocal()
            }
        }
    }
}