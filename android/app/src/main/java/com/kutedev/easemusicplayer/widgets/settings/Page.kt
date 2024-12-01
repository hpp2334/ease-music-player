package com.kutedev.easemusicplayer.widgets.settings

import android.content.pm.PackageManager
import android.os.Build
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R


private val paddingX = 24.dp

private fun getAppVersion(
    context: android.content.Context,
): String {
    val packageManager = context.packageManager
    val packageName = context.packageName
    val packageInfo = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        packageManager.getPackageInfo(packageName, PackageManager.PackageInfoFlags.of(0))
    } else {
        packageManager.getPackageInfo(packageName, 0)
    }
    return packageInfo.versionName ?: "<unknown>"
}

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
    content: String,
    onClick: () -> Unit
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .padding(0.dp, 4.dp)
            .fillMaxWidth()
            .clickable { onClick() }
    ) {
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
            Text(
                text = content,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontSize = 12.sp,
            )
        }
    }
}

@Composable
fun SettingSubpage(ctx: android.content.Context) {
    val uriHandler = LocalUriHandler.current
    val gitUrl = "https://github.com/hpp2334/ease-music-player";

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(paddingX, paddingX)
            .verticalScroll(rememberScrollState())
    ) {
        Title(title = stringResource(id = R.string.setting_about))
        Item(
            iconPainter = painterResource(R.drawable.icon_github),
            title = stringResource(id = R.string.setting_git_repo),
            content = gitUrl,
            onClick = {
                uriHandler.openUri(gitUrl)
            }
        )
        Item(
            iconPainter = painterResource(R.drawable.icon_info),
            title = stringResource(id = R.string.setting_version),
            content = getAppVersion(ctx),
            onClick = {}
        )
    }
}