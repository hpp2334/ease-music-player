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
import uniffi.ease_client_backend.DataSourceKey

@Composable
fun EaseImage(
    modifier: Modifier = Modifier,
    dataSourceKey: DataSourceKey,
    contentScale: ContentScale
) {
    var oldKey by remember { mutableStateOf(DataSourceKeyH(dataSourceKey)) }
    val key = DataSourceKeyH(dataSourceKey)
    var bitmap = null

    if (oldKey != key) {
        oldKey = key;
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