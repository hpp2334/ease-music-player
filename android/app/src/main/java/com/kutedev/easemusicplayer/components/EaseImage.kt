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
    // `remember(keyH)` resets the slot on a data-source change within the
    // same composition. An unkeyed remember kept rendering the PREVIOUS
    // key's bitmap until the effect swapped it — one stale frame when the
    // new asset is cached, and the whole previous cover during a slow
    // remote fetch (the doc below promises nothing-while-loading, not
    // wrong-cover-while-loading).
    var state: AssetBitmap? by remember(keyH) { mutableStateOf(vm.getCachedAsset(dataSourceKey)) }

    LaunchedEffect(keyH) {
        if (state == null) {
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
