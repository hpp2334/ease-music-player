package com.kutedev.easemusicplayer.widgets.devices

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.ConfirmDialog
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.FormWidget
import com.kutedev.easemusicplayer.singleton.StorageProvider
import com.kutedev.easemusicplayer.turintegration.EasePluginBridge
import com.kutedev.easemusicplayer.turintegration.TurView
import com.kutedev.easemusicplayer.viewmodels.EditStorageVM
import com.kutedev.easemusicplayer.core.LocalNavController
import androidx.hilt.navigation.compose.hiltViewModel


private fun buildStr(s: String): AnnotatedString {
    val spans = s.split("$$")

    return buildAnnotatedString {
        for (s in spans) {
            if (s.startsWith("B__")) {
                val s = s.slice("B__".length until s.length)

                withStyle(style = SpanStyle(
                    fontWeight = FontWeight(700)
                )) {
                    append(s)
                }
            } else {
                append(s)
            }
        }
    }
}

@Composable
private fun RemoveDialog(
    editStorageVM: EditStorageVM = hiltViewModel()
) {
    val navController = LocalNavController.current
    val title by editStorageVM.title.collectAsState()
    val musicCount by editStorageVM.musicCount.collectAsState()
    val isOpen by editStorageVM.removeModalOpen.collectAsState()

    val mainDesc = buildStr(
        stringResource(R.string.storage_remove_desc_main)
            .replace("E_TITLE", title)
    )
    val countDesc = buildStr(
        stringResource(R.string.storage_remove_desc_count)
            .replace("E_MCNT", musicCount.toString())
    )

    ConfirmDialog(
        open = isOpen,
        onConfirm = {
            editStorageVM.closeRemoveModal()
            editStorageVM.remove()
            navController.popBackStack()
        },
        onCancel = {
            editStorageVM.closeRemoveModal()
        },
    ) {
        Text(
            text = mainDesc,
            fontSize = 14.sp
        )
        Text(
            text = countDesc,
            fontSize = 14.sp
        )
    }
}

/** Storage-provider card in the chooser. Non-selectable cards (other
 *  providers in edit mode — a storage can't change its type) render dimmed. */
@Composable
private fun StorageBlock(
    title: String,
    isActive: Boolean,
    disabled: Boolean = false,
    onSelect: () -> Unit
) {
    val bgColor = if (isActive) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceVariant
    val tint = if (isActive) MaterialTheme.colorScheme.surface else MaterialTheme.colorScheme.onSurface
    val dim = if (disabled) 0.4f else 1f

    Box(
        modifier = Modifier
            .size(100.dp)
            .clip(RoundedCornerShape(20.dp))
            .background(bgColor.copy(alpha = dim))
            .clickable(enabled = !disabled) { onSelect() }
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.align(Alignment.Center)
        ) {
            Icon(
                painter = painterResource(id = R.drawable.icon_cloud),
                contentDescription = null,
                tint = tint.copy(alpha = dim),
            )
            Text(
                text = title,
                color = tint.copy(alpha = dim),
            )
        }
    }
}

/**
 * Hosts a plugin storage's view JS in a [TurView]. The plugin owns all
 * config UI (alias / connection fields, "Connect your account", test +
 * save buttons, edit-view disconnect, ...). Providers with an OAuth flow
 * trigger it via `ease.oauth.new()` + `ease.oauth.start(oauthId)` (business
 * data like the alias stays in the plugin, keyed by the flow id);
 * non-OAuth providers (e.g. WebDAV) persist their instance from the view
 * via a backend RPC + `ease.context.createStorage`.
 *
 * [pluginId] is stamped into the instance's per-instance data slot so
 * `ease:*` bridge fns resolve the calling plugin from Rust. [sourceHandle]
 * is the view's module-source handle (registered Rust-side by the
 * `plugin.list` scan — the JS bytes never cross JNI). [instance] is the
 * storage's `plugin_storage_id` for edit views (non-null →
 * `ease.context.storageId$` reports the id; null = create-mode setup
 * view).
 */
@Composable
private fun PluginStorageView(
    pluginId: String,
    sourceHandle: Long,
    instance: String?,
) {
    val context = LocalContext.current

    when {
        sourceHandle == 0L -> Text(
            text = "Plugin view load failed: no source handle",
            fontSize = 14.sp,
            color = MaterialTheme.colorScheme.error,
        )
        else -> TurView(
            runtime = EasePluginBridge.runtime(context),
            sourceHandle = sourceHandle,
            pluginId = pluginId,
            instance = instance,
            modifier = Modifier
                .fillMaxWidth()
                .height(480.dp),
        )
    }
}

@Composable
fun EditStoragesPage(
    editStorageVM: EditStorageVM = hiltViewModel()
) {
    val navController = LocalNavController.current
    val isCreated by editStorageVM.isCreated.collectAsState()
    val pluginMode by editStorageVM.pluginMode.collectAsState()
    val providers by editStorageVM.storageProviders.collectAsState()
    val editPluginView by editStorageVM.editPluginView.collectAsState()
    val title by editStorageVM.title.collectAsState()

    // When a plugin registers a new storage instance (the OAuth redirect
    // handled by `MainActivity`, or a non-OAuth backend RPC followed by
    // `ease.context.createStorage`), pop back from the setup form.
    //
    // The short delay lets the setup view's engine quiesce (pending rpc
    // replies + pump posts drain) before disposal — destroying the TurView
    // mid-pump races the engine's loop driver (use-after-free in
    // `pump_loop`, see the crash repro'd during the WebDAV connect test).
    LaunchedEffect(Unit) {
        editStorageVM.pluginConnectedEvent.collect {
            kotlinx.coroutines.delay(250)
            navController.popBackStack()
        }
    }
    // Pop back when the edited storage is removed (view-side disconnect via
    // the plugin backend, or the top-bar trash).
    LaunchedEffect(Unit) {
        editStorageVM.removedEvent.collect { navController.popBackStack() }
    }

    // Create-mode chooser selection: one card per discovered plugin
    // storage provider (WebDAV, OneDrive, ...). In edit mode the provider
    // is fixed by the loaded storage's handle (`pluginMode`).
    var selectedProvider by remember { mutableStateOf<StorageProvider?>(null) }
    val activeProvider = if (isCreated) selectedProvider else null
    val showPlugin = if (isCreated) selectedProvider != null else pluginMode

    Column(
        modifier = Modifier
            .background(MaterialTheme.colorScheme.surface)
            .fillMaxSize()
    ) {
        Row(
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .padding(12.dp)
                .fillMaxWidth()
        ) {
            Row {
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Default,
                    painter = painterResource(id = R.drawable.icon_back),
                    onClick = {
                        navController.popBackStack()
                    }
                )
            }
            Row {
                if (!isCreated) {
                    EaseIconButton(
                        sizeType = EaseIconButtonSize.Medium,
                        buttonType = EaseIconButtonType.Error,
                        painter = painterResource(id = R.drawable.icon_deleteseep),
                        onClick = {
                            editStorageVM.openRemoveModal()
                        }
                    )
                }
            }
        }
        Box(
            modifier = Modifier.fillMaxSize()
        ) {
            Column(
                verticalArrangement = Arrangement.spacedBy(10.dp),
                modifier = Modifier
                    .verticalScroll(rememberScrollState())
                    .imePadding()
                    .padding(30.dp, 12.dp)
            ) {
                // Storage-type chooser: one card per discovered plugin
                // storage provider (WebDAV, OneDrive, ...). Shown in both
                // modes — create picks the provider to set up; edit marks
                // the storage's own provider (fixed — a storage can't
                // change its type).
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (p in providers) {
                        val active = if (isCreated) {
                            selectedProvider?.storageId == p.storageId
                        } else {
                            editPluginView?.pluginId == p.pluginId
                        }
                        StorageBlock(
                            title = p.displayName,
                            isActive = active,
                            disabled = !isCreated && !active,
                            onSelect = { if (isCreated) selectedProvider = p }
                        )
                    }
                }
                Box(modifier = Modifier.size(0.dp, 20.dp))
                if (showPlugin) {
                    if (isCreated) {
                        // Create-mode chooser selection.
                        val handle = activeProvider?.viewSourceHandle ?: 0L
                        val pid = activeProvider?.pluginId
                        if (pid != null) {
                            PluginStorageView(pid, handle, instance = null)
                        }
                    } else {
                        // Edit mode: render the storage's plugin view,
                        // stamped with its plugin_storage_id so
                        // `ease.context.storageId$` is non-null (edit branch).
                        val epv = editPluginView
                        if (epv != null) {
                            PluginStorageView(
                                pluginId = epv.pluginId,
                                sourceHandle = epv.viewSourceHandle,
                                instance = epv.pluginStorageId,
                            )
                        } else {
                            // Provider not resolved yet (scanPlugins pending)
                            // — show the alias as a static fallback.
                            FormWidget(label = stringResource(R.string.storage_edit_oauth)) {
                                Text(
                                    text = title.ifBlank { "Plugin" },
                                    fontSize = 14.sp,
                                    color = MaterialTheme.colorScheme.onSurface,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
    RemoveDialog()
}
