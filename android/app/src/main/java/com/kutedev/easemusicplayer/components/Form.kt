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
import androidx.compose.runtime.collectAsState
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
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow


interface IFormTextFieldState {
    val error: StateFlow<Int?>
    val value: StateFlow<String>
    fun update(value: String): Unit
}

class FormTextFieldState(
    private val initValue: String,
    private val validator: ((value: String) -> Int?)?
): IFormTextFieldState {
    private val _error = MutableStateFlow<Int?>(null)
    private val _value = MutableStateFlow(initValue)

    override val error = _error.asStateFlow()
    override val value = _value.asStateFlow()

    override fun update(value: String) {
        _value.value = value

        if (validator != null) {
            _error.value = null
        }
    }

    fun validate(): Boolean {
        if (validator != null) {
            _error.value = validator!!(_value.value)
        }
        return _error.value == null
    }
}


@Composable
fun SimpleFormText(
    label: String,
    value: String,
    onChange: (value: String) -> Unit
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
            onValueChange = {value -> onChange(value)},
        )
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