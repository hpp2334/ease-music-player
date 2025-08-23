package com.kutedev.easemusicplayer.viewmodels

import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.singleton.AssetRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import uniffi.ease_client_schema.DataSourceKey
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
