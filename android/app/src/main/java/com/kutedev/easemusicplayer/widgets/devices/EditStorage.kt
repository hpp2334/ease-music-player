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
import androidx.compose.runtime.rememberCoroutineScope
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
import com.kutedev.easemusicplayer.components.EaseIconButtonColors
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.components.EaseTextButton
import com.kutedev.easemusicplayer.components.EaseTextButtonSize
import com.kutedev.easemusicplayer.components.EaseTextButtonType
import com.kutedev.easemusicplayer.components.FormSwitch
import com.kutedev.easemusicplayer.components.FormText
import com.kutedev.easemusicplayer.components.FormWidget
import com.kutedev.easemusicplayer.singleton.StorageProvider
import com.kutedev.easemusicplayer.turintegration.EasePluginBridge
import com.kutedev.easemusicplayer.turintegration.TurView
import com.kutedev.easemusicplayer.viewmodels.EditStorageVM
import com.kutedev.easemusicplayer.core.LocalNavController
import com.kutedev.easemusicplayer.singleton.types.StorageConnectionTestResult
import androidx.hilt.navigation.compose.hiltViewModel
import android.content.Intent
import android.net.Uri
import kotlinx.coroutines.launch


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

/** Selectable storage-type card in the create-mode chooser. */
@Composable
private fun StorageBlock(
    title: String,
    isActive: Boolean,
    onSelect: () -> Unit
) {
    val bgColor = if (isActive) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceVariant
    val tint = if (isActive) MaterialTheme.colorScheme.surface else MaterialTheme.colorScheme.onSurface

    Box(
        modifier = Modifier
            .size(100.dp)
            .clip(RoundedCornerShape(20.dp))
            .background(bgColor)
            .clickable { onSelect() }
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.align(Alignment.Center)
        ) {
            Icon(
                painter = painterResource(id = R.drawable.icon_cloud),
                contentDescription = null,
                tint = tint,
            )
            Text(
                text = title,
                color = tint,
            )
        }
    }
}

@Composable
private fun WebdavConfig(
    editStorageVM: EditStorageVM = hiltViewModel()
) {
    val form by editStorageVM.form.collectAsState()
    val validated by editStorageVM.validated.collectAsState()
    val isAnonymous = form.isAnonymous

    FormSwitch(
        label = stringResource(id = R.string.storage_edit_anonymous),
        value = isAnonymous,
        onChange = { editStorageVM.updateForm { storage ->
            storage.isAnonymous = !storage.isAnonymous
            storage
        }}
    )
    FormText(
        label = stringResource(id = R.string.storage_edit_alias),
        value = form.alias,
        onChange = { value -> editStorageVM.updateForm { storage ->
            storage.alias = value
            storage
        } },
    )
    FormText(
        label = stringResource(id = R.string.storage_edit_addr),
        value = form.addr,
        onChange = { value -> editStorageVM.updateForm { storage ->
            storage.addr = value
            storage
        } },
        error = if (validated.addrEmpty) {
            R.string.storage_edit_form_address
        } else {
            null
        }
    )
    if (!isAnonymous) {
        FormText(
            label = stringResource(id = R.string.storage_edit_username),
            value = form.username,
            onChange = { value -> editStorageVM.updateForm { storage ->
                storage.username = value
                storage
            } },
            error = if (validated.usernameEmpty) {
                R.string.storage_edit_form_username
            } else {
                null
            }
        )
        FormText(
            label = stringResource(id = R.string.storage_edit_password),
            value = form.password,
            isPassword = true,
            onChange = { value -> editStorageVM.updateForm { storage ->
                storage.password = value
                storage
            } },
            error = if (validated.passwordEmpty) {
                R.string.storage_edit_form_password
            } else {
                null
            }
        )
    }
}

/**
 * Hosts a plugin storage's view JS in a [TurView]. The plugin owns all
 * config UI (alias field, "Connect your account" button, …) and triggers
 * OAuth via `ease.oauth.start(provider, alias)`; the host fetches the
 * authorize URL, stashes the alias, and opens the browser.
 *
 * [assetPath] is the absolute asset path to the plugin's view JS bundle
 * (e.g. `plugins/com.ease.onedrive/view.js`). [pluginId] is stamped into
 * the instance's per-instance data slot so `ease:*` bridge fns resolve the
 * calling plugin from Rust.
 */
@Composable
private fun PluginStorageView(assetPath: String, pluginId: String) {
    val context = LocalContext.current
    var jsSource by remember(assetPath) { mutableStateOf<String?>(null) }
    var loadError by remember(assetPath) { mutableStateOf<String?>(null) }

    LaunchedEffect(assetPath) {
        loadError = null
        jsSource = runCatching {
            context.assets.open(assetPath).bufferedReader().use { it.readText() }
        }.getOrElse {
            loadError = it.message ?: "unknown error"
            null
        }
    }

    when {
        loadError != null -> Text(
            text = "Plugin view load failed: $loadError",
            fontSize = 14.sp,
            color = MaterialTheme.colorScheme.error,
        )
        jsSource == null -> Text(
            text = "Loading…",
            fontSize = 14.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        else -> TurView(
            runtime = EasePluginBridge.runtime(context),
            js = jsSource!!,
            pluginId = pluginId,
            modifier = Modifier
                .fillMaxWidth()
                .height(360.dp),
        )
    }
}

@Composable
fun EditStoragesPage(
    editStorageVM: EditStorageVM = hiltViewModel()
) {
    val navController = LocalNavController.current
    val coroutineScope = rememberCoroutineScope()
    val isCreated by editStorageVM.isCreated.collectAsState()
    val pluginMode by editStorageVM.pluginMode.collectAsState()
    val providers by editStorageVM.storageProviders.collectAsState()
    val title by editStorageVM.title.collectAsState()
    val testing by editStorageVM.testResult.collectAsState()

    // When a plugin OAuth exchange succeeds (handled by `MainActivity` via
    // the `easem://oauth2redirect` callback), pop back from the setup form.
    LaunchedEffect(Unit) {
        editStorageVM.pluginConnectedEvent.collect { navController.popBackStack() }
    }

    // Create-mode chooser selection: `null` = WebDAV, else the selected
    // plugin provider. In edit mode the type is fixed by the loaded
    // storage's handle (`pluginMode`).
    var selectedProvider by remember { mutableStateOf<StorageProvider?>(null) }
    val activeProvider = if (isCreated) selectedProvider else null
    val showPlugin = if (isCreated) selectedProvider != null else pluginMode

    val testingColors = when (testing) {
        StorageConnectionTestResult.NONE -> null
        StorageConnectionTestResult.TESTING -> EaseIconButtonColors(
            buttonBg = Color.Transparent,
            iconTint = MaterialTheme.colorScheme.tertiary,
        )
        StorageConnectionTestResult.SUCCESS -> EaseIconButtonColors(
            buttonBg = Color.Transparent,
            iconTint = MaterialTheme.colorScheme.primary,
        )
        else -> EaseIconButtonColors(
            buttonBg = Color.Transparent,
            iconTint = MaterialTheme.colorScheme.error,
        )
    }

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
                // Save / test only apply to WebDAV. Plugin storages are
                // created via the setup view's OAuth flow + redirect, so
                // neither button is shown in plugin mode.
                if (!showPlugin) {
                    EaseIconButton(
                        sizeType = EaseIconButtonSize.Medium,
                        buttonType = EaseIconButtonType.Default,
                        disabled = testing == StorageConnectionTestResult.TESTING,
                        painter = painterResource(id = R.drawable.icon_wifitethering),
                        overrideColors = testingColors,
                        onClick = {
                            editStorageVM.test()
                        }
                    )
                    EaseIconButton(
                        sizeType = EaseIconButtonSize.Medium,
                        buttonType = EaseIconButtonType.Default,
                        painter = painterResource(id = R.drawable.icon_ok),
                        onClick = {
                            coroutineScope.launch {
                                val finished = editStorageVM.finish()
                                if (finished) {
                                    navController.popBackStack()
                                }
                            }
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
                if (isCreated) {
                    // Storage-type chooser: WebDAV (built-in) + one card per
                    // discovered plugin storage provider.
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        StorageBlock(
                            title = "WebDAV",
                            isActive = selectedProvider == null,
                            onSelect = { selectedProvider = null }
                        )
                        for (p in providers) {
                            StorageBlock(
                                title = p.displayName,
                                isActive = selectedProvider?.storageId == p.storageId,
                                onSelect = { selectedProvider = p }
                            )
                        }
                    }
                    Box(modifier = Modifier.size(0.dp, 20.dp))
                }
                if (showPlugin) {
                    val assetPath = activeProvider?.viewAssetPath
                    val pid = activeProvider?.pluginId
                    if (assetPath != null && pid != null) {
                        PluginStorageView(assetPath, pid)
                    } else {
                        // Edit mode for a plugin storage: already connected —
                        // show its alias; removal is via the top-bar trash.
                        FormWidget(label = stringResource(R.string.storage_edit_oauth)) {
                            Text(
                                text = title.ifBlank { "Plugin" },
                                fontSize = 14.sp,
                                color = MaterialTheme.colorScheme.onSurface,
                            )
                        }
                    }
                } else {
                    WebdavConfig()
                }
            }
        }
    }
    RemoveDialog()
}
