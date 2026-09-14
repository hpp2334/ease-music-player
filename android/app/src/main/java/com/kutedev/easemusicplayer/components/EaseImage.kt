import androidx.compose.foundation.Image
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.core.DataSourceKeyH
import com.kutedev.easemusicplayer.singleton.AssetBitmap
import com.kutedev.easemusicplayer.viewmodels.AssetVM
import com.kutedev.easemusicplayer.singleton.types.DataSourceKey

/**
 * Renders the bitmap behind a [DataSourceKey]. Three terminal states:
 * loaded bitmap, no cover default art ([fallback], shown for
 * [AssetBitmap.Failed] — bytes missing or undecodable; the repository
 * caches that outcome terminally), or nothing while the load is still in
 * flight.
 */
@Composable
fun EaseImage(
    modifier: Modifier = Modifier,
    dataSourceKey: DataSourceKey,
    contentScale: ContentScale,
    fallback: Painter = painterResource(R.drawable.cover_default_image),
    vm: AssetVM = hiltViewModel()
) {
    val keyH = DataSourceKeyH(dataSourceKey)
    var oldKey: DataSourceKeyH by remember { mutableStateOf(keyH) }
    var state: AssetBitmap? by remember { mutableStateOf(vm.getCachedAsset(dataSourceKey)) }

    LaunchedEffect(keyH, state == null) {
        if (keyH != oldKey || state == null) {
            oldKey = keyH
            state = vm.loadAsset(keyH.value())
        }
    }

    when (val s = state) {
        // Still resolving — render nothing rather than the fallback, so
        // a slow remote fetch doesn't flash default art first.
        null -> return
        is AssetBitmap.Loaded -> Image(
            modifier = modifier,
            bitmap = s.bitmap,
            contentDescription = null,
            contentScale = contentScale,
        )
        AssetBitmap.Failed -> Image(
            modifier = modifier,
            painter = fallback,
            contentDescription = null,
            contentScale = contentScale,
        )
    }
}
