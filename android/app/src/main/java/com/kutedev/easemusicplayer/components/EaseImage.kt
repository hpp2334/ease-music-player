import androidx.compose.foundation.Image
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import com.kutedev.easemusicplayer.core.DataSourceKeyH
import com.kutedev.easemusicplayer.core.UIBridgeController
import uniffi.ease_client_shared.DataSourceKey

@Composable
fun EaseImage(
    modifier: Modifier = Modifier,
    dataSourceKey: DataSourceKey,
    contentScale: ContentScale
) {
    val bridge = UIBridgeController.current
    var oldKey by remember { mutableStateOf(DataSourceKeyH(dataSourceKey)) }
    val key = DataSourceKeyH(dataSourceKey)
    var bitmap by remember { mutableStateOf(bridge.bitmapDataSources.get(key)) }

    if (oldKey != key) {
        oldKey = key;
        bitmap = bridge.bitmapDataSources.get(key)
    }

    LaunchedEffect(key) {
        val data = bridge.bitmapDataSources.load(key)
        bitmap = data
    }

    if (bitmap == null) {
        return
    }

    Image(
        modifier = modifier,
        bitmap = bitmap!!,
        contentDescription = null,
        contentScale = contentScale,
    )
}