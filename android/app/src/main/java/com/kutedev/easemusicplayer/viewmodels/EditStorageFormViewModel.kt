package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.core.IOnNotifyView
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client.EditStorageFormValidated
import uniffi.ease_client.FormFieldStatus
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.VCurrentPlaylistState
import uniffi.ease_client.VEditStorageState
import uniffi.ease_client_shared.ArgUpsertStorage
import uniffi.ease_client_shared.StorageConnectionTestResult
import uniffi.ease_client_shared.StorageId
import uniffi.ease_client_shared.StorageType

class EditStorageFormViewModel(): ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VEditStorageState(
            isCreated = true,
            title = "",
            info = ArgUpsertStorage(
                id = null,
                addr = "",
                alias = "",
                username = "",
                password = "",
                isAnonymous = true,
                typ = StorageType.WEBDAV,
            ),
            validated = EditStorageFormValidated(
                address = FormFieldStatus.OK,
                username = FormFieldStatus.OK,
                password = FormFieldStatus.OK
            ),
            test = StorageConnectionTestResult.NONE,
            musicCount = 0u,
            playlistCount = 0u,
        )
    })
    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.editStorage == null) {
            return
        }

        _state.value = v.editStorage!!.copy()
    }
}