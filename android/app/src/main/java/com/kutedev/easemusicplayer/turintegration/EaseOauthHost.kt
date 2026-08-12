package com.kutedev.easemusicplayer.turintegration

/**
 * JNI upcall target for the Rust `ease:oauth` host module.
 *
 * The `ease:oauth.start(provider, alias)` bridge (in
 * `rust-libs/.../plugin_runtime/oauth_bridge.rs`) resolves the process
 * `JavaVM` via `ndk_context` and calls
 * `EaseOauthHost.startOauth(provider, alias)` as a static method from the
 * engine thread (the Android main looper, where view instances pump). This
 * object is a thin static shell — it delegates to an [OauthHandler] installed
 * once at startup ([MainActivity.onCreate]), which owns the Hilt-injected
 * dependencies (repositories, app scope, context).
 *
 * The `alias` argument arrives as a plain `String`; a blank value means
 * "no alias" (the plugin defaults it).
 */
object EaseOauthHost {
    @Volatile
    private var handler: OauthHandler? = null

    /** Install the runtime handler. Called once from `MainActivity.onCreate`. */
    fun install(h: OauthHandler) {
        handler = h
    }

    /**
     * `ease:oauth.start(provider, alias)` upcall entry. Fire-and-forget:
     * launches the OAuth flow (fetch URL → stash → browser) on the app
     * scope; the redirect callback in `MainActivity.onNewIntent` completes
     * the exchange.
     */
    @JvmStatic
    fun startOauth(provider: String, alias: String) {
        val h = handler ?: run {
            android.util.Log.w("EaseOauthHost", "startOauth ignored: handler not installed")
            return
        }
        h.startOauth(provider, alias.ifBlank { null })
    }
}
