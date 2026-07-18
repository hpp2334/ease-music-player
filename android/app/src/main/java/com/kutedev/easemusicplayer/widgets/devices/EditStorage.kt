package com.kutedev.easemusicplayer.widgets.devices

import android.content.Intent
import android.content.Intent.FLAG_ACTIVITY_NEW_TASK
import android.net.Uri
import android.widget.Toast
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
import androidx.lifecycle.viewmodel.compose.viewModel
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
import com.kutedev.easemusicplayer.viewmodels.EditStorageVM
import com.kutedev.easemusicplayer.core.LocalNavController
import kotlinx.coroutines.flow.update
import uniffi.ease_client_backend.StorageConnectionTestResult
import uniffi.ease_client_backend.ctOnedriveOauthUrl
import uniffi.ease_client_schema.StorageType
import androidx.core.net.toUri
import androidx.hilt.navigation.compose.hiltViewModel
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgUpsertStorage


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

@Composable
private fun StorageBlock(
    title: String,
    isActive: Boolean,
    onSelect: () -> Unit
) {
    val bgColor = if (isActive) { MaterialTheme.colorScheme.primary } else { MaterialTheme.colorScheme.surfaceVariant }
    val tint = if (isActive) { MaterialTheme.colorScheme.surface } else { MaterialTheme.colorScheme.onSurface }

    Box(
        modifier = Modifier
            .size(100.dp)
            .clip(RoundedCornerShape(20.dp))
            .background(bgColor)
            .clickable { onSelect() }
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier
                .align(Alignment.Center)
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
    val isAnonymous = form.isAnonymous;

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

@Composable
private fun OneDriveConfig(
    editStorageVM: EditStorageVM = hiltViewModel()
) {
    val context = LocalContext.current
    val form by editStorageVM.form.collectAsState()
    val validated by editStorageVM.validated.collectAsState()
    val connected = form.password.isNotEmpty()

    FormText(
        label = stringResource(id = R.string.storage_edit_alias),
        value = form.alias,
        onChange = { value -> editStorageVM.updateForm { storage ->
            storage.alias = value
            storage
        } },
        error = if (validated.aliasEmpty) {
            R.string.storage_edit_onedrive_alias_not_empty
        } else {
            null
        }
    )
    FormWidget(
        label = stringResource(R.string.storage_edit_oauth)
    ) {
        if (!connected) {
            EaseTextButton(
                text = stringResource(R.string.storage_edit_onedrive_connect),
                type = EaseTextButtonType.PrimaryVariant,
                size = EaseTextButtonSize.Medium,
                onClick = {
                    val intent = Intent(Intent.ACTION_VIEW, ctOnedriveOauthUrl().toUri())
                    intent.flags = FLAG_ACTIVITY_NEW_TASK
                    context.startActivity(intent)
                },
            )
            if (validated.passwordEmpty) {
                Text(
                    modifier = Modifier.padding(
                        horizontal = 0.dp,
                        vertical = 2.dp,
                    ),
                    text = stringResource(R.string.storage_edit_onedrive_should_auth),
                    color = MaterialTheme.colorScheme.error,
                    fontSize = 11.sp,
                )
            }
        }
        if (connected) {
            EaseTextButton(
                text = stringResource(R.string.storage_edit_onedrive_disconnect),
                type = EaseTextButtonType.Error,
                size = EaseTextButtonSize.Medium,
                onClick = {
                    editStorageVM.updateForm { storage ->
                        storage.password = ""
                        storage
                    }
                },
            )
        }
    }
}

@Composable
fun EditStoragesPage(
    editStorageVM: EditStorageVM = hiltViewModel()
) {
    val navController = LocalNavController.current
    val coroutineScope = rememberCoroutineScope()
    val form by editStorageVM.form.collectAsState();
    val isCreated by editStorageVM.isCreated.collectAsState();
    val testing by editStorageVM.testResult.collectAsState()

    val storageType = form.typ;

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
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    StorageBlock(
                        title = "WebDAV",
                        isActive = storageType == StorageType.WEBDAV,
                        onSelect = {
                            editStorageVM.changeType(StorageType.WEBDAV)
                        }
                    )
                    StorageBlock(
                        title = "OneDrive",
                        isActive = storageType == StorageType.ONE_DRIVE,
                        onSelect = {
                            editStorageVM.changeType(StorageType.ONE_DRIVE)
                        }
                    )
                }
                Box(modifier = Modifier.height(30.dp))
                if (storageType == StorageType.WEBDAV) {
                    WebdavConfig()
                }
                if (storageType == StorageType.ONE_DRIVE) {
                    OneDriveConfig()
                }
            }
        }
    }
    RemoveDialog()
}
