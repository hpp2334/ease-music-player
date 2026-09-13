package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.singleton.StorageRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import com.kutedev.easemusicplayer.singleton.types.StorageEntry
import com.kutedev.easemusicplayer.singleton.types.StorageEntryType

@HiltViewModel
class StoragesVM @Inject constructor(
    private val storageRepository: StorageRepository,
) : ViewModel() {
    val storages = storageRepository.storages

    suspend fun reload() {
        storageRepository.reload()
    }
}

val MUSIC_EXTS = arrayOf(".wav", ".mp3", ".aac", ".flac", ".ogg", ".m4a")
val IMAGE_EXTS = arrayOf(".jpg", ".jpeg", ".png")

/**
 * Lyric extensions recognized by the browse/import filters — fed from the
 * plugin parser registry by `PluginRepository.scanPlugins()`
 * (`setLyricExts`). There is no built-in parser: the set is empty until
 * an enabled plugin declares `contributions.lyricParsers`.
 */
@Volatile
private var lyricExts: Set<String> = emptySet()

/** Published by [com.kutedev.easemusicplayer.singleton.PluginRepository]
 *  on every plugin scan. */
fun setLyricExts(exts: Set<String>) {
    lyricExts = exts.map { it.lowercase() }.toSet()
}

/** Current lyric extensions WITH the leading dot (filter form). */
fun currentLyricExts(): List<String> = lyricExts.map { ".$it" }

fun StorageEntry.entryTyp(): StorageEntryType {
    if (isDir) {
        return StorageEntryType.FOLDER
    }
    val lowerPath = path.lowercase()
    return when {
        MUSIC_EXTS.any { lowerPath.endsWith(it) } -> StorageEntryType.MUSIC
        IMAGE_EXTS.any { lowerPath.endsWith(it) } -> StorageEntryType.IMAGE
        lyricExts.any { lowerPath.endsWith(".${it}") } -> StorageEntryType.LYRIC
        else -> StorageEntryType.OTHER
    }
}
