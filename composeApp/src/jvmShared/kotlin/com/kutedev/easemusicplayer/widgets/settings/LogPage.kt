package com.kutedev.easemusicplayer.widgets.settings

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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.viewmodels.LogVM
import com.kutedev.easemusicplayer.viewmodels.PlaylistsVM
import com.kutedev.easemusicplayer.platform.platformOpenFile
import org.koin.compose.viewmodel.koinViewModel
import org.jetbrains.compose.resources.painterResource
import org.jetbrains.compose.resources.pluralStringResource
import org.jetbrains.compose.resources.stringResource
import easemusicplayer.composeapp.generated.resources.Res
import easemusicplayer.composeapp.generated.resources.*

private val paddingX = SettingPaddingX

@Composable
fun LogPage(
    logVM: LogVM = koinViewModel()
) {
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
                text = stringResource(Res.string.log_title),
                fontSize = 32.sp,
            )
            Text(
                modifier = Modifier.padding(horizontal = paddingX),
                text = pluralStringResource(Res.plurals.log_desc, logs.size, logs.size),
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
                                platformOpenFile(log.path)
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