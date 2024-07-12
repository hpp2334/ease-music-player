package com.kutedev.easemusicplayer.viewmodels

import androidx.compose.ui.res.stringResource
import androidx.compose.ui.util.fastAll
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.core.IOnNotifyView
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.ease_client.ArgUpsertStorage
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.StorageConnectionTestResult
import uniffi.ease_client.StorageId
import uniffi.ease_client.StorageType
import uniffi.ease_client.upsertStorage

interface IFormTextFieldState {
    val error: StateFlow<Int?>
    val value: StateFlow<String>
    fun update(value: String): Unit
}

private class FormTextFieldState(
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

class EditStorageFormViewModel(): ViewModel(), IOnNotifyView {
    private var _lastUpdateSignal: UShort = 0u
    private var _storageId: StorageId? = null
    private var _testing = MutableStateFlow(StorageConnectionTestResult.NONE)
    private val _isCreated = MutableStateFlow(true)
    private val _isAnonymous = MutableStateFlow(false)
    private val _storageType = MutableStateFlow(StorageType.WEBDAV)
    private val _alias = FormTextFieldState(
        "",
        validator = null,
    )
    private val _address = FormTextFieldState(
        "",
        validator = {
            value -> if (value.isEmpty()) {
                R.string.storage_edit_form_address
            } else {
                null
            }
        }
    )
    private val _username = FormTextFieldState(
        "",
        validator = {
            value -> if (value.isEmpty() && !_isAnonymous.value) {
                R.string.storage_edit_form_username
            } else {
                null
            }
        }
    )
    private val _password = FormTextFieldState(
        "",
        validator = {
            value -> if (value.isEmpty() && !_isAnonymous.value) {
                R.string.storage_edit_form_password
            } else {
                null
            }
        }
    )

    val testing = _testing
    val isCreated = _isCreated
    val isAnonymous = _isAnonymous
    val storageType = _storageType
    val alias: IFormTextFieldState = _alias
    val address: IFormTextFieldState = _address
    val username: IFormTextFieldState = _username
    val password: IFormTextFieldState = _password

    fun updateIsAnonymous(value: Boolean) {
        _isAnonymous.value = value
    }
    fun updateStorageType(value: StorageType) {
        _storageType.value = value
    }

    fun validate(): Boolean {
        val allValidated = listOf(
            _alias.validate(),
            _address.validate(),
            _username.validate(),
            _password.validate()
        )
        return allValidated.all { x -> x }
    }

    fun validateAndGetSubmit(): ArgUpsertStorage? {
        if (!validate()) {
            return null
        }

        return ArgUpsertStorage(
            id = _storageId,
            addr = _address.value.value,
            alias = _alias.value.value,
            username = _username.value.value,
            password = _password.value.value,
            isAnonymous = _isAnonymous.value,
            typ = _storageType.value,
        )
    }

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.editStorage == null) {
            return
        }

        val state = v.editStorage!!.copy()
        _testing.value = state.test
        if (state.updateSignal != _lastUpdateSignal) {
            _lastUpdateSignal = state.updateSignal

            _storageId = state.info.id
            _isCreated.value = state.isCreated
            _isAnonymous.value = state.info.isAnonymous
            _storageType.value = state.info.typ
            _alias.update(state.info.alias ?: "")
            _address.update(state.info.addr)
            _username.update(state.info.username)
            _password.update(state.info.password)
        }
    }
}