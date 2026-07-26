package com.kutedev.easemusicplayer.widgets.plugins

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RoutePlugin
import com.kutedev.easemusicplayer.singleton.PluginViewItem
import com.kutedev.easemusicplayer.viewmodels.PluginsVM

private val pluginsPaddingX = 24.dp

@Composable
fun PluginsSubpage(
    pluginsVM: PluginsVM = hiltViewModel(),
) {
    val navController = LocalNavController.current
    val views by pluginsVM.pluginViews.collectAsState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
    ) {
        Box(modifier = Modifier.height(48.dp))
        Row(
            modifier = Modifier
                .padding(pluginsPaddingX, 4.dp)
                .fillMaxWidth(),
        ) {
            Text(
                text = stringResource(id = R.string.plugins_title),
                color = MaterialTheme.colorScheme.primary,
                fontSize = 14.sp,
            )
        }

        if (views.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(pluginsPaddingX, 24.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                Text(
                    text = stringResource(id = R.string.plugins_empty),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 14.sp,
                )
            }
        } else {
            for (item in views) {
                PluginViewRow(item) {
                    navController.navigate(RoutePlugin(item.pluginId, item.viewId))
                }
            }
        }
    }
}

@Composable
private fun PluginViewRow(
    item: PluginViewItem,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(pluginsPaddingX, 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(
                modifier = Modifier.size(32.dp),
                painter = painterResource(id = R.drawable.icon_extension),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
            )
            Box(modifier = Modifier.width(20.dp))
            Column {
                Text(
                    text = item.viewTitle,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Medium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = item.pluginName,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 12.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
