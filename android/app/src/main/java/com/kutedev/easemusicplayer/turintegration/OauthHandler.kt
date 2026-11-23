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
 * [EaseOauthHost] (the `ease:oauth.start(oauthId)` Rust→Kotlin upcall —
 * the plugin's identity rides the call, the flow token is the only arg).
 *
 * Mirrors what the former Kotlin `OneDriveConfig` composable did inline, but
 * is plugin-agnostic: fetch the authorize URL from the headless service RPC,
 * stash `(pluginId, oauthId)` in [PluginOAuthState] so the
 * `easem://oauth2redirect` callback can finish the exchange, then open the
 * system browser. Business data (the alias) never crosses this layer — the
 * plugin keys it by the same `oauthId` in its own KV.
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
    fun startOauth(pluginId: String, oauthId: String) {
        scope.launch {
            val url = storageRepository.pluginOAuthUrl(pluginId, oauthId) ?: run {
                android.util.Log.w("OauthHandler", "pluginOAuthUrl($pluginId) returned null")
                return@launch
            }
            pluginOAuthState.set(pluginId, oauthId)
            context.startActivity(
                Intent(Intent.ACTION_VIEW, Uri.parse(url))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
        }
    }
}
