package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.repositories.StorageRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.Storage
import uniffi.ease_client_backend.StorageEntry
import uniffi.ease_client_backend.StorageEntryType
import uniffi.ease_client_schema.StorageId

@HiltViewModel
class StoragesVM @Inject constructor(
    private val storageRepository: StorageRepository
) : ViewModel() {
    val storages = storageRepository.storages

    suspend fun reload() {
        storageRepository.reload()
    }
}

val MUSIC_EXTS = arrayOf(".wav", ".mp3", ".aac", ".flac", ".ogg", ".m4a")
val IMAGE_EXTS = arrayOf(".jpg", ".jpeg", ".png")
val LYRIC_EXTS = arrayOf(".lrc")

fun StorageEntry.entryTyp(): StorageEntryType {
    if (isDir) {
        return StorageEntryType.FOLDER
    }
    val lowerPath = path.lowercase()
    return when {
        MUSIC_EXTS.any { lowerPath.endsWith(it) } -> StorageEntryType.MUSIC
        IMAGE_EXTS.any { lowerPath.endsWith(it) } -> StorageEntryType.IMAGE
        LYRIC_EXTS.any { lowerPath.endsWith(it) } -> StorageEntryType.LYRIC
        else -> StorageEntryType.OTHER
    }
}

