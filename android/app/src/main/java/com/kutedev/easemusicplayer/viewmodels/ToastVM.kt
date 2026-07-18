package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.singleton.StorageRepository
import com.kutedev.easemusicplayer.singleton.ToastRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject


@HiltViewModel
class ToastVM @Inject constructor(
    private val toastRepository: ToastRepository
) : ViewModel() {
    val toast = toastRepository.toast
    val toastRes = toastRepository.toastRes
}
