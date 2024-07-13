package com.kutedev.easemusicplayer.widgets.devices

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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.LocalNavController
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.components.EaseIconButton
import com.kutedev.easemusicplayer.components.EaseIconButtonColors
import com.kutedev.easemusicplayer.components.EaseIconButtonSize
import com.kutedev.easemusicplayer.components.EaseIconButtonType
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.viewmodels.EditStorageFormViewModel
import com.kutedev.easemusicplayer.viewmodels.IFormTextFieldState
import uniffi.ease_client.StorageConnectionTestResult
import uniffi.ease_client.StorageType
import uniffi.ease_client.testConnection
import uniffi.ease_client.upsertStorage


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
    field: IFormTextFieldState,
    isPassword: Boolean = false
) {
    var passwordVisibleState = remember { mutableStateOf(false) }
    val passwordVisible = passwordVisibleState.value
    val value = field.value.collectAsState().value
    val error = field.error.collectAsState().value

    Column(
        modifier = Modifier
            .fillMaxWidth()
    ) {
        Text(
            text = label,
            fontSize = 10.sp,
            letterSpacing = 1.sp,
        )
        if (!isPassword) {
            TextField(
                modifier = Modifier
                    .fillMaxWidth(),
                value = value,
                onValueChange = {value -> field.update(value)},
                isError = error != null,
            )
        } else {
            TextField(
                modifier = Modifier
                    .fillMaxWidth(),
                value = value,
                onValueChange = {value -> field.update(value)},
                isError = error != null,
                visualTransformation = if (passwordVisible) VisualTransformation.None else PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                trailingIcon = {
                    val painter = if (!passwordVisible) {
                        painterResource(id = R.drawable.icon_visibility)
                    } else {
                        painterResource(id = R.drawable.icon_visibility_off)
                    }

                    IconButton(onClick = {
                        passwordVisibleState.value = !passwordVisible
                    }) {
                        Icon(painter = painter, contentDescription = null)
                    }
                }
            )
        }
        if (error != null) {
            Text(
                text =  stringResource(id = error),
                color = MaterialTheme.colorScheme.error,
                fontSize = 10.sp,
            )
        }
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
    formVM: EditStorageFormViewModel,
) {
    val navController = LocalNavController.current
//
//    var formStorageType by remember {
//        mutableStateOf(state.info.typ)
//    }
//    var formIsAnonymous by remember {
//        mutableStateOf(state.info.isAnonymous)
//    }
//    var formAlias by remember {
//        mutableStateOf(state.info.alias ?: "")
//    }
//    var formAddr by remember {
//        mutableStateOf(state.info.addr)
//    }
//    var formUsername by remember {
//        mutableStateOf(state.info.username)
//    }
//    var formPassword by remember {
//        mutableStateOf(state.info.password)
//    }
//
//    LaunchedEffect(state.updateSignal) {
//        formStorageType = state.info.typ
//        formIsAnonymous = state.info.isAnonymous
//        formAlias = state.info.alias ?: ""
//        formAddr = state.info.addr
//        formUsername = state.info.username
//        formPassword = state.info.password
//    }

    val context = LocalContext.current

    val toast = remember {
        Toast.makeText(context, "", Toast.LENGTH_SHORT)
    }

    val isCreated = formVM.isCreated.collectAsState().value
    val isAnonymous = formVM.isAnonymous.collectAsState().value
    val storageType = formVM.storageType.collectAsState().value
    val testing = formVM.testing.collectAsState().value

    val testingColors = when (testing) {
        StorageConnectionTestResult.NONE -> null
        StorageConnectionTestResult.TESTING -> EaseIconButtonColors(
            Color.Transparent,
            MaterialTheme.colorScheme.tertiary,
        )
        StorageConnectionTestResult.SUCCESS -> EaseIconButtonColors(
            Color.Transparent,
            MaterialTheme.colorScheme.primary,
        )
        else -> EaseIconButtonColors(
            Color.Transparent,
            MaterialTheme.colorScheme.error,
        )
    }

    LaunchedEffect(testing) {
        println(testing)
        if (testing == StorageConnectionTestResult.NONE || testing == StorageConnectionTestResult.TESTING) {
            return@LaunchedEffect;
        }

        when (testing) {
            StorageConnectionTestResult.SUCCESS -> {
                toast.setText(R.string.storage_edit_testing_toast_success)
            }
            StorageConnectionTestResult.TIMEOUT -> {
                toast.setText(R.string.storage_edit_testing_toast_timeout)
            }
            StorageConnectionTestResult.UNAUTHORIZED -> {
                toast.setText(R.string.storage_edit_testing_toast_unauth)
            }
            StorageConnectionTestResult.OTHER_ERROR -> {
                toast.setText(R.string.storage_edit_testing_toast_other_error)
            }
            else -> {}
        }
        toast.cancel()
        toast.show()
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
                if (!isCreated) {
                    EaseIconButton(
                        sizeType = EaseIconButtonSize.Medium,
                        buttonType = EaseIconButtonType.Error,
                        painter = painterResource(id = R.drawable.icon_deleteseep),
                        onClick = {}
                    )
                }
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Default,
                    painter = painterResource(id = R.drawable.icon_wifitethering),
                    overrideColors = testingColors,
                    onClick = {
                        val value = formVM.validateAndGetSubmit()
                        println(value)
                        if (value != null) {
                            Bridge.invoke {
                                testConnection(value)
                            }
                        }
                    }
                )
                EaseIconButton(
                    sizeType = EaseIconButtonSize.Medium,
                    buttonType = EaseIconButtonType.Default,
                    painter = painterResource(id = R.drawable.icon_ok),
                    onClick = {
                        val value = formVM.validateAndGetSubmit()
                        if (value != null) {
                            Bridge.invoke {
                                upsertStorage(value)
                            }
                            navController.popBackStack()
                        }
                    }
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
                    isActive = storageType == StorageType.WEBDAV,
                    onSelect = {
                        formVM.updateStorageType(StorageType.WEBDAV)
                    }
                )
            }
            Box(modifier = Modifier.height(30.dp))
            FormSwitch(
                label = stringResource(id = R.string.storage_edit_anonymous),
                value = isAnonymous,
                onChange = { value -> formVM.updateIsAnonymous(value) }
            )
            FormText(
                label = stringResource(id = R.string.storage_edit_alias),
                field = formVM.alias,
            )
            FormText(
                label = stringResource(id = R.string.storage_edit_addr),
                field = formVM.address,
            )
            if (!isAnonymous) {
                FormText(
                    label = stringResource(id = R.string.storage_edit_username),
                    field = formVM.username,
                )
                FormText(
                    label = stringResource(id = R.string.storage_edit_password),
                    field = formVM.password,
                    isPassword = true,
                )
            }
        }

    }
}