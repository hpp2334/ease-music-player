package com.kutedev.easemusicplayer.widgets.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.platform.LocalUriHandler
import easemusicplayer.shared.generated.resources.Res
import easemusicplayer.shared.generated.resources.icon_github
import easemusicplayer.shared.generated.resources.icon_info
import easemusicplayer.shared.generated.resources.icon_log
import easemusicplayer.shared.generated.resources.icon_vertialcal_more
import easemusicplayer.shared.generated.resources.setting_about
import easemusicplayer.shared.generated.resources.setting_debug
import easemusicplayer.shared.generated.resources.setting_git_repo
import easemusicplayer.shared.generated.resources.setting_log
import easemusicplayer.shared.generated.resources.setting_more
import easemusicplayer.shared.generated.resources.setting_version
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RouteDebugMore
import com.kutedev.easemusicplayer.core.RouteLog

private val paddingX = SettingPaddingX

@Composable
expect fun getAppVersion(): String


@Composable
private fun Title(title: String) {
    Column {
        Text(
            text = title,
            letterSpacing = 1.sp,
            fontSize = 14.sp,
        )
        Box(
           modifier = Modifier
               .fillMaxWidth()
               .height(1.dp)
               .background(MaterialTheme.colorScheme.onSurfaceVariant)
        )
    }
}

@Composable
private fun Item(
    iconPainter: Painter,
    title: String,
    content: String?,
    onClick: () -> Unit
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() }
    ) {
        Box(modifier = Modifier.height(56.dp))
        Icon(
            painter = iconPainter,
            contentDescription = null,
            modifier = Modifier
                .size(24.dp)
        )
        Box(
            modifier = Modifier.width(12.dp)
        )
        Column {
            Text(
                text = title,
                fontSize = 14.sp,
            )
            if (content != null) {
                Text(
                    text = content,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 12.sp,
                )
            }
        }
    }
}
@Composable
fun SettingSubpage() {
    val uriHandler = LocalUriHandler.current
    val gitUrl = "https://github.com/hpp2334/ease-music-player";
    val navController = LocalNavController.current

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(paddingX, paddingX)
            .verticalScroll(rememberScrollState())
    ) {
        Title(title = stringResource(Res.string.setting_debug))
        Item(
            iconPainter = painterResource(Res.drawable.icon_log),
            title = stringResource(Res.string.setting_log),
            content = null,
            onClick = {
                navController.navigate(RouteLog())
            }
        )
        Item(
            iconPainter = painterResource(Res.drawable.icon_vertialcal_more),
            title = stringResource(Res.string.setting_more),
            content = null,
            onClick = {
                navController.navigate(RouteDebugMore())
            }
        )
        Title(title = stringResource(Res.string.setting_about))
        Item(
            iconPainter = painterResource(Res.drawable.icon_github),
            title = stringResource(Res.string.setting_git_repo),
            content = gitUrl,
            onClick = {
                uriHandler.openUri(gitUrl)
            }
        )
        Item(
            iconPainter = painterResource(Res.drawable.icon_info),
            title = stringResource(Res.string.setting_version),
            content = getAppVersion(),
            onClick = {}
        )
    }
}
