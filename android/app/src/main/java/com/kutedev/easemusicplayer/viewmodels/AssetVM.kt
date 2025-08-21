package com.kutedev.easemusicplayer.viewmodels

import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.repositories.AssetRepository
import com.kutedev.easemusicplayer.repositories.ImportRepository
import com.kutedev.easemusicplayer.repositories.PlaylistRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import uniffi.ease_client_backend.ctGetAsset
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_schema.DataSourceKey
import uniffi.ease_client_schema.StorageEntryLoc
import javax.inject.Inject


@HiltViewModel
class AssetVM @Inject constructor(
    private val assetRepository: AssetRepository
) : ViewModel() {

    suspend fun load(key: DataSourceKey): ByteArray? {
        return assetRepository.load(key)
    }
    suspend fun loadBitmap(key: DataSourceKey): ImageBitmap? {
        return assetRepository.loadBitmap(key)
    }
    fun get(key: DataSourceKey): ByteArray? {
        return assetRepository.get(key)
    }
    fun getBitmap(key: DataSourceKey): ImageBitmap? {
        return assetRepository.getBitmap(key)
    }
}
