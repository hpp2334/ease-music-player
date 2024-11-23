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
import com.kutedev.easemusicplayer.core.UIBridgeController
import uniffi.ease_client_shared.DataSourceKey

@Composable
fun EaseImage(
    modifier: Modifier = Modifier,
    dataSourceKey: DataSourceKey,
    contentScale: ContentScale
) {
    val bridge = UIBridgeController.current
    var bitmap by remember { mutableStateOf(bridge.bitmapDataSources.get(dataSourceKey)) }

    LaunchedEffect(dataSourceKey) {
        val data = bridge.bitmapDataSources.load(dataSourceKey)
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