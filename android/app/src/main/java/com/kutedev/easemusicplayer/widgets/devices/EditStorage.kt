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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.viewmodels.EditStorageViewModel
import uniffi.ease_client.StorageType

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
fun FormText(
    label: String,
    value: String,
    onChange: (value: String) -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
    ) {
        Text(
            text = label,
            fontSize = 10.sp,
            letterSpacing = 1.sp,
        )
        TextField(
            modifier = Modifier
                .fillMaxWidth(),
            value = value,
            onValueChange = onChange
        )
    }
}

@Composable
fun FormSwitch(
    label: String,
    value: Boolean,
    onChange: (value: Boolean) -> Unit,
) {
    Column {
        Text(
            text = label,
            fontSize = 10.sp,
            letterSpacing = 1.sp,
        )
        Switch(
            checked = value,
            onCheckedChange = onChange
        )
    }
}

@Composable
fun EditStoragesPage(
    editStorageVM: EditStorageViewModel
) {
    val state = editStorageVM.state.collectAsState().value;
    val navController = LocalNavController.current

    var formStorageType by remember {
        mutableStateOf(state.info.typ)
    }
    var formIsAnonymous by remember {
        mutableStateOf(state.info.isAnonymous)
    }
    var formAlias by remember {
        mutableStateOf(state.info.alias ?: "")
    }
    var formAddr by remember {
        mutableStateOf(state.info.addr)
    }
    var formUsername by remember {
        mutableStateOf(state.info.username)
    }
    var formPassword by remember {
        mutableStateOf(state.info.password)
    }

    LaunchedEffect(state.updateSignal) {
        formStorageType = state.info.typ
        formIsAnonymous = state.info.isAnonymous
        formAlias = state.info.alias ?: ""
        formAddr = state.info.addr
        formUsername = state.info.username
        formPassword = state.info.password
    }

    Column(
        modifier = Modifier
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
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Error,
                    painter = painterResource(id = R.drawable.icon_deleteseep),
                    onClick = {}
                )
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Default,
                    painter = painterResource(id = R.drawable.icon_wifitethering),
                    onClick = {}
                )
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Default,
                    painter = painterResource(id = R.drawable.icon_ok),
                    onClick = {}
                )
            }
        }
        Column(
            verticalArrangement = Arrangement.spacedBy(10.dp),
            modifier = Modifier
                .padding(30.dp, 12.dp)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
        ) {
            Row {
                StorageBlock(
                    title = "WebDAV",
                    isActive = state.info.typ == StorageType.WEBDAV,
                    onSelect = {
                        formStorageType = StorageType.WEBDAV
                    }
                )
            }
            Box(modifier = Modifier.height(30.dp))
            FormSwitch(
                label = stringResource(id = R.string.storage_edit_anonymous),
                value = formIsAnonymous,
                onChange = { value -> formIsAnonymous = value; }
            )
            FormText(
                label = stringResource(id = R.string.storage_edit_alias),
                value = formAlias,
                onChange = { value -> formAlias = value; }
            )
            FormText(
                label = stringResource(id = R.string.storage_edit_addr),
                value = formAddr,
                onChange = { value -> formAddr = value; }
            )
            if (!formIsAnonymous) {
                FormText(
                    label = stringResource(id = R.string.storage_edit_username),
                    value = formUsername,
                    onChange = { value -> formUsername = value; }
                )
                FormText(
                    label = stringResource(id = R.string.storage_edit_password),
                    value = formPassword,
                    onChange = { value -> formPassword = value; }
                )
            }
        }

    }
}