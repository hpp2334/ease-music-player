package com.kutedev.easemusicplayer.singleton

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Whether track artist lines are shown in the UI (the secondary line
 * under a track title in playlist rows, and the now-playing subtitle),
 * persisted **backend-side** in the `preference` table
 * (`preference.getShowTrackArtist` / `preference.saveShowTrackArtist`
 * over the JSON bridge, via the ordinary suspend [Bridge.call]) — same
 * owner as [LanguageSetting]. Nothing is written or read synchronously
 * from Kotlin:
 *
 * - [show] is the process-lifetime [StateFlow] view of the backend
 *   value, seeded `true` and hydrated by [load] at app start. The
 *   backend column also ships `DEFAULT 1`, so a preference row that
 *   predates the column (or doesn't exist yet) still shows artists.
 * - The lock-screen `MediaMetadataCompat.METADATA_KEY_ARTIST` is NOT
 *   gated by this — it always carries the real artist (playlist title
 *   as fallback), independent of in-app display preferences.
 *
 * Stays a plain `object` (not a Hilt repository) so the Application can
 * install the bridge exactly like it does for [LanguageSetting].
 */
object ArtistDisplaySetting {
    /** Installed by [com.kutedev.easemusicplayer.MainActivity]'s application class; null only before app start. */
    @Volatile
    internal var bridge: Bridge? = null

    private val _show = MutableStateFlow(true)

    /** Whether artist lines are shown; backend-backed, seeded on. */
    val show: StateFlow<Boolean> = _show.asStateFlow()

    /**
     * Hydrate [show] from the backend. Fire-and-forget safe: failures
     * leave the flow at its previous value (the bridge logs the error).
     */
    suspend fun load() {
        val b = bridge ?: return
        val show = b.call(BridgeMethods.Preference.GET_SHOW_TRACK_ARTIST)
            .unwrapOrNull()
            ?.payload
        if (show != null) {
            _show.value = show
        }
    }

    /**
     * Persist [show] to the backend and publish it to [show]. No-op-ish
     * on failure (returns false, error logged through the bridge).
     */
    suspend fun save(show: Boolean): Boolean {
        val b = bridge ?: return false
        val ret = b.call(BridgeMethods.Preference.SAVE_SHOW_TRACK_ARTIST, show)
        if (!ret.isSuccess) {
            ret.unwrapOrNull() // logs the failure detail
            return false
        }
        _show.value = show
        return true
    }
}
