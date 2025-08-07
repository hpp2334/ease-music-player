package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.Storage
import javax.inject.Inject


@HiltViewModel
class StorageVM @Inject constructor() : ViewModel() {
    private val _storages = MutableStateFlow(listOf<Storage>())
    val storages = _storages.asStateFlow()
}
