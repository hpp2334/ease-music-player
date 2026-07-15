package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.singleton.StorageRepository
import com.kutedev.easemusicplayer.singleton.ToastRepository


class ToastVM constructor(
    private val toastRepository: ToastRepository
) : ViewModel() {
    val toast = toastRepository.toast
    val toastRes = toastRepository.toastRes
}
