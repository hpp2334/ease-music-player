package com.kutedev.easemusicplayer

import android.app.Application
import com.kutedev.easemusicplayer.di.androidModule
import com.kutedev.easemusicplayer.di.appModule
import com.kutedev.easemusicplayer.platform.appContext
import com.kutedev.easemusicplayer.platform.initPlatformContext
import org.koin.core.context.startKoin

class EaseMusicPlayerApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        initPlatformContext(this)
        startKoin {
            modules(appModule, androidModule)
        }
    }
}
