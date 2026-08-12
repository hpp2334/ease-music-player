package com.kutedev.easemusicplayer.turintegration

import android.content.Context
import android.content.Intent
import android.net.Uri
import com.kutedev.easemusicplayer.singleton.PluginOAuthState
import com.kutedev.easemusicplayer.singleton.StorageRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Handles OAuth-start requests fired by plugin setup views through
 * [EaseOauthHost] (the `ease:oauth.start` Rust→Kotlin upcall).
 *
 * Mirrors what the former Kotlin `OneDriveConfig` composable did inline, but
 * is now generic over `provider`: fetch the authorize URL from the headless
 * service RPC, stash `(provider, alias)` in [PluginOAuthState] so the
 * `easem://oauth2redirect` callback can finish the exchange, then open the
 * system browser.
 *
 * The browser launch runs on the app [CoroutineScope] (not an Activity), so
 * the [Intent] carries [Intent.FLAG_ACTIVITY_NEW_TASK].
 */
@Singleton
class OauthHandler @Inject constructor(
    private val storageRepository: StorageRepository,
    private val pluginOAuthState: PluginOAuthState,
    private val scope: CoroutineScope,
    @ApplicationContext private val context: Context,
) {
    fun startOauth(provider: String, alias: String?) {
        scope.launch {
            val url = storageRepository.pluginOAuthUrl(provider) ?: run {
                android.util.Log.w("OauthHandler", "pluginOAuthUrl($provider) returned null")
                return@launch
            }
            pluginOAuthState.set(provider, alias)
            context.startActivity(
                Intent(Intent.ACTION_VIEW, Uri.parse(url))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
        }
    }
}
