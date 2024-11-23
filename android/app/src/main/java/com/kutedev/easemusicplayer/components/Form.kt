package com.kutedev.easemusicplayer.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.sp
import com.kutedev.easemusicplayer.R

@Composable
fun SimpleFormText(
    label: String?,
    value: String,
    onChange: (value: String) -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
    ) {
        if (label != null) {
            Text(
                text = label,
                fontSize = 10.sp,
                letterSpacing = 1.sp,
            )
        }
        TextField(
            modifier = Modifier
                .fillMaxWidth(),
            value = value,
            onValueChange = {value -> onChange(value)},
        )
    }
}

@Composable
fun FormText(
    label: String,
    value: String,
    onChange: (value: String) -> Unit,
    error: Int? = null,
    isPassword: Boolean = false
) {
    var passwordVisibleState = remember { mutableStateOf(false) }
    val passwordVisible = passwordVisibleState.value

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
                onValueChange = onChange,
                isError = error != null,
            )
        } else {
            TextField(
                modifier = Modifier
                    .fillMaxWidth(),
                value = value,
                onValueChange = onChange,
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
                text = stringResource(id = error),
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