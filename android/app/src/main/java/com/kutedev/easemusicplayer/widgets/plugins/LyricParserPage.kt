package com.kutedev.easemusicplayer.widgets.plugins

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalMinimumInteractiveComponentEnforcement
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
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
import com.kutedev.easemusicplayer.components.PluginIconBox
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.core.RoutePluginAvailable
import com.kutedev.easemusicplayer.singleton.LyricParserContribution
import com.kutedev.easemusicplayer.singleton.resolve
import com.kutedev.easemusicplayer.viewmodels.LyricParserGroup
import com.kutedev.easemusicplayer.viewmodels.LyricParserVM
import com.kutedev.easemusicplayer.viewmodels.selectionKey

private val parsersPaddingX = 24.dp

/**
 * Lyric Parser management: one group per extension claimed by an enabled
 * plugin's parser, each with an Auto (default dispatch order) default and
 * a user pick of which plugin's parser wins. There is no built-in parser
 * — LRC included — so with no parser plugin the page shows the empty
 * hint + a shortcut to the plugin registry.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LyricParserPage(
    scaffoldPadding: PaddingValues,
    vm: LyricParserVM = hiltViewModel(),
) {
    val navController = LocalNavController.current
    val groups by vm.groups.collectAsState()
    val empty by vm.empty.collectAsState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(top = scaffoldPadding.calculateTopPadding())
    ) {
        // Top bar
        CompositionLocalProvider(LocalMinimumInteractiveComponentEnforcement provides false) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(56.dp)
                    .padding(parsersPaddingX, 0.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(
                    modifier = Modifier.size(40.dp),
                    onClick = { navController.popBackStack() },
                ) {
                    Icon(
                        modifier = Modifier.size(20.dp),
                        painter = painterResource(id = R.drawable.icon_back),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurface,
                    )
                }
                Box(modifier = Modifier.width(12.dp))
                Text(
                    text = stringResource(id = R.string.lyric_parser_title),
                    fontSize = 18.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = parsersPaddingX)
        ) {
            if (empty) {
                Box(modifier = Modifier.height(24.dp))
                Text(
                    text = stringResource(id = R.string.lyric_parser_hint),
                    fontSize = 14.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Box(modifier = Modifier.height(16.dp))
                Text(
                    text = stringResource(id = R.string.lyric_parser_get_plugins),
                    fontSize = 14.sp,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier
                        .clip(RoundedCornerShape(10.dp))
                        .clickable { navController.navigate(RoutePluginAvailable()) }
                        .padding(8.dp, 10.dp),
                )
            }
            for (group in groups) {
                Box(modifier = Modifier.height(16.dp))
                Text(
                    text = ".${group.ext}",
                    fontSize = 14.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Box(modifier = Modifier.height(4.dp))
                // Auto row — the default (and the effective state for a
                // stale pick). No plugin icon.
                SelectRow(
                    selected = group.selectedKey == null,
                    label = stringResource(id = R.string.lyric_parser_auto),
                    sub = null,
                    iconData = null,
                    showIcon = false,
                    onClick = { vm.select(group.ext, null) },
                )
                for (candidate in group.candidates) {
                    // Always show which plugin the parser comes from —
                    // the parser title alone (e.g. "SRT / WebVTT") doesn't
                    // identify the source when several plugins claim the
                    // same extension.
                    val parserTitle = candidate.title?.resolve()
                    val subParts = mutableListOf<String>()
                    if (parserTitle != null) {
                        subParts.add(candidate.pluginName.resolve())
                    }
                    candidate.desc?.resolve()?.let { subParts.add(it) }
                    subParts.add(candidate.extensions.joinToString(" ") { ".$it" })
                    SelectRow(
                        selected = group.selectedKey == candidate.selectionKey(),
                        label = parserTitle ?: candidate.pluginName.resolve(),
                        sub = subParts.joinToString("  ·  "),
                        iconData = candidate.iconData,
                        showIcon = true,
                        onClick = { vm.select(group.ext, candidate.selectionKey()) },
                    )
                }
            }
            Box(modifier = Modifier.height(24.dp))
        }
    }
}

@Composable
private fun SelectRow(
    selected: Boolean,
    label: String,
    sub: String?,
    iconData: String?,
    showIcon: Boolean,
    onClick: () -> Unit,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .clickable { onClick() }
            .padding(8.dp, 10.dp),
    ) {
        Text(
            text = if (selected) "● " else "○ ",
            fontSize = 12.sp,
            color = MaterialTheme.colorScheme.primary,
        )
        if (showIcon) {
            PluginIconBox(
                iconData = iconData,
                tint = MaterialTheme.colorScheme.primary,
                boxSize = 36.dp,
                glyphSize = 18.dp,
            )
            Box(modifier = Modifier.width(10.dp))
        }
        Column {
            Text(text = label, fontSize = 14.sp)
            if (sub != null) {
                Text(
                    text = sub,
                    fontSize = 12.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
