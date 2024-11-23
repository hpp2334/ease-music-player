package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.core.IOnNotifyView
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client.EditStorageFormValidated
import uniffi.ease_client.FormFieldStatus
import uniffi.ease_client.RootRouteSubKey
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.VCreatePlaylistState
import uniffi.ease_client.VCurrentMusicLyricState
import uniffi.ease_client.VCurrentMusicState
import uniffi.ease_client.VCurrentPlaylistState
import uniffi.ease_client.VCurrentStorageEntriesState
import uniffi.ease_client.VEditPlaylistState
import uniffi.ease_client.VEditStorageState
import uniffi.ease_client.VMainState
import uniffi.ease_client.VPlaylistListState
import uniffi.ease_client.VStorageListState
import uniffi.ease_client.VTimeToPauseState
import uniffi.ease_client_shared.ArgUpsertStorage
import uniffi.ease_client_shared.CreatePlaylistMode
import uniffi.ease_client_shared.CurrentStorageImportType
import uniffi.ease_client_shared.CurrentStorageStateType
import uniffi.ease_client_shared.LyricLoadState
import uniffi.ease_client_shared.PlayMode
import uniffi.ease_client_shared.StorageConnectionTestResult
import uniffi.ease_client_shared.StorageType


val DefaultPlaylistListState = VPlaylistListState(emptyList())
val DefaultMainState = VMainState(subkey = RootRouteSubKey.PLAYLIST, vsLoaded = false)
val DefaultTimeToPauseState =
        VTimeToPauseState(enabled = false, leftHour = 0u, leftMinute = 0u, modalOpen = false)
val DefaultStorageListState = VStorageListState(items = emptyList())
val DefaultCurrentPlaylistState =
        VCurrentPlaylistState(
                id = null,
                items = emptyList(),
                title = "",
                duration = "",
                cover = null,
        )
val DefaultCurrentMusicState =
        VCurrentMusicState(
                id = null,
                title = "",
                currentDuration = "",
                totalDuration = "",
                currentDurationMs = 0UL,
                totalDurationMs = 0UL,
                canChangePosition = false,
                canPlayNext = false,
                canPlayPrevious = false,
                previousCover = null,
                nextCover = null,
                cover = null,
                playMode = PlayMode.SINGLE,
                playing = false,
                lyricIndex = 0,
                loading = false
        )
val DefaultCurrentStorageEntriesState =
        VCurrentStorageEntriesState(
                importType = CurrentStorageImportType.None,
                stateType = CurrentStorageStateType.LOADING,
                currentStorageId = null,
                storageItems = emptyList(),
                entries = emptyList(),
                selectedCount = 0,
                splitPaths = emptyList(),
                currentPath = "",
                disabledToggleAll = false,
                canUndo = false
        )
val DefaultEditStorageState =
        VEditStorageState(
                isCreated = true,
                title = "",
                info =
                        ArgUpsertStorage(
                                id = null,
                                addr = "",
                                alias = "",
                                username = "",
                                password = "",
                                isAnonymous = true,
                                typ = StorageType.WEBDAV
                        ),
                validated =
                        EditStorageFormValidated(
                                address = FormFieldStatus.OK,
                                username = FormFieldStatus.OK,
                                password = FormFieldStatus.OK
                        ),
                test = StorageConnectionTestResult.NONE,
                musicCount = 0u,
                playlistCount = 0u
        )
val DefaultCreatePlaylistState =
        VCreatePlaylistState(
                mode = CreatePlaylistMode.FULL,
                name = "",
                picture = null,
                musicCount = 0u,
                recommendPlaylistNames = emptyList(),
                fullImported = false,
                modalOpen = false,
                canSubmit = false
        )
val DefaultEditPlaylistState = VEditPlaylistState(name = "", picture = null, modalOpen = false)
val DefaultCurrentMusicLyricState =
        VCurrentMusicLyricState(lyricLines = listOf(), loadState = LyricLoadState.LOADING)

class EaseViewModel : ViewModel(), IOnNotifyView {
    private val _playlistListState = MutableStateFlow(DefaultPlaylistListState)
    val playlistListState = _playlistListState.asStateFlow()

    private val _mainState = MutableStateFlow(DefaultMainState)
    val mainState = _mainState.asStateFlow()

    private val _timeToPauseState = MutableStateFlow(DefaultTimeToPauseState)
    val timeToPauseState = _timeToPauseState.asStateFlow()

    private val _storageListState = MutableStateFlow(DefaultStorageListState)
    val storageListState = _storageListState.asStateFlow()

    private val _currentPlaylistState = MutableStateFlow(DefaultCurrentPlaylistState)
    val currentPlaylistState = _currentPlaylistState.asStateFlow()

    private val _currentMusicState = MutableStateFlow(DefaultCurrentMusicState)
    val currentMusicState = _currentMusicState.asStateFlow()

    private val _currentStorageEntriesState = MutableStateFlow(DefaultCurrentStorageEntriesState)
    val currentStorageEntriesState = _currentStorageEntriesState.asStateFlow()

    private val _editStorageState = MutableStateFlow(DefaultEditStorageState)
    val editStorageState = _editStorageState.asStateFlow()

    private val _createPlaylistState = MutableStateFlow(DefaultCreatePlaylistState)
    val createPlaylistState = _createPlaylistState.asStateFlow()

    private val _editPlaylistState = MutableStateFlow(DefaultEditPlaylistState)
    val editPlaylistState = _editPlaylistState.asStateFlow()

    private val _currentMusicLyricState = MutableStateFlow(DefaultCurrentMusicLyricState)
    val currentMusicLyricState = _currentMusicLyricState.asStateFlow()

    override fun onNotifyView(v: RootViewModelState) {
        v.playlistList?.let { _playlistListState.value = it.copy() }
        v.currentRouter?.let { _mainState.value = it.copy() }
        v.timeToPause?.let { _timeToPauseState.value = it.copy() }
        v.storageList?.let { _storageListState.value = it.copy() }
        v.currentPlaylist?.let { _currentPlaylistState.value = it.copy() }
        v.currentMusic?.let { _currentMusicState.value = it.copy() }
        v.currentStorageEntries?.let { _currentStorageEntriesState.value = it.copy() }
        v.editStorage?.let { _editStorageState.value = it.copy() }
        v.createPlaylist?.let { _createPlaylistState.value = it.copy() }
        v.editPlaylist?.let { _editPlaylistState.value = it.copy() }
        v.currentMusicLyric?.let { _currentMusicLyricState.value = it.copy() }
    }
}
