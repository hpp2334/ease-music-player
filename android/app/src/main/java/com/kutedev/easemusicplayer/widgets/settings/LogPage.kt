package com.kutedev.easemusicplayer.widgets.settings

import android.content.Intent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import androidx.core.net.toUri
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.viewmodels.LogVM
import com.kutedev.easemusicplayer.viewmodels.PlaylistsVM
import java.io.File

private val paddingX = SettingPaddingX

@Composable
fun LogPage(
    logVM: LogVM = hiltViewModel()
) {
    val context = LocalContext.current
    val logs by logVM.logs.collectAsState()

    LaunchedEffect(Unit) {
        logVM.reload()
    }

    Box(
        modifier = Modifier.fillMaxSize(),
    ) {
        Column {
            Text(
                modifier = Modifier.padding(start = paddingX, end = paddingX, top = 24.dp, bottom = 4.dp),
                text = stringResource(id = R.string.log_title),
                fontSize = 32.sp,
            )
            Text(
                modifier = Modifier.padding(horizontal = paddingX),
                text = pluralStringResource(id = R.plurals.log_desc, count = logs.size, logs.size),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontSize = 14.sp,
            )
            Box(modifier = Modifier.height(24.dp))
            LazyColumn(
                modifier = Modifier.weight(1.0f)
            ) {
                items(logs) { log ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable {
                                val file = File(log.path)
                                val uri = FileProvider.getUriForFile(
                                    context,
                                    "${context.packageName}.fileprovider", // Manifest 里配置的 authority
                                    file
                                )
                                val intent = Intent(Intent.ACTION_VIEW).apply {
                                    setDataAndType(uri, "text/plain")
                                    flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK
                                }
                                context.startActivity(intent)
                            }
                    ) {
                        Text(
                            modifier = Modifier
                                .padding(horizontal = paddingX, vertical = 8.dp),
                            text = log.name,
                            fontFamily = FontFamily.Monospace
                        )
                    }
                }
            }
            Box(modifier = Modifier.height(24.dp))
        }
    }
}