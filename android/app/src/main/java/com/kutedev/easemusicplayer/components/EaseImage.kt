import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.core.DataSourceKeyH
import com.kutedev.easemusicplayer.viewmodels.AssetVM
import uniffi.ease_client_schema.DataSourceKey

@Composable
fun EaseImage(
    modifier: Modifier = Modifier,
    dataSourceKey: DataSourceKey,
    contentScale: ContentScale,
    vm: AssetVM = hiltViewModel()
) {
    var oldKey: DataSourceKeyH by remember { mutableStateOf(DataSourceKeyH(dataSourceKey)) }
    var bitmap: ImageBitmap? by remember { mutableStateOf(vm.getBitmap(dataSourceKey)) }
    val key = DataSourceKeyH(dataSourceKey)

    LaunchedEffect(key.hashCode(), bitmap != null) {
        if (key != oldKey || bitmap == null) {
            oldKey = key
            bitmap = vm.loadBitmap(key.value())
        }
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
