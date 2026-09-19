package com.kutedev.easemusicplayer.singleton

import android.content.Context
import android.content.res.Configuration
import android.os.LocaleList
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * In-app language override options shown in Settings → General.
 *
 * [ENGLISH] forces `values/`, [SIMPLIFIED_CHINESE] forces `values-zh-rCN/`;
 * [SYSTEM] leaves resolution to the OS locale.
 */
enum class AppLanguage(val tag: String?) {
    SYSTEM(null),
    ENGLISH("en"),
    SIMPLIFIED_CHINESE("zh-CN"),
}

/**
 * In-app language preference, persisted **backend-side** in the
 * `preference` table (`preference.getLanguage` / `preference.saveLanguage`
 * over the JSON bridge, via the ordinary suspend [Bridge.call]) — same
 * owner as PlayMode. Nothing is written or read synchronously from Kotlin:
 *
 * - [language] is the process-lifetime [StateFlow] view of the backend
 *   value, seeded `SYSTEM` and hydrated by [load] at app start.
 * - [attachBaseContext] wraps the activity's base context with whatever
 *   the flow currently holds (runs before injection, so it only reads the
 *   cache) and records it in [applied].
 * - [com.kutedev.easemusicplayer.MainActivity] collects [language]; when
 *   it disagrees with [applied] — either because the async [load] landed
 *   after the first frame, or because the user picked a new language —
 *   the activity `recreate()`s and the next `attachBaseContext` applies
 *   the new locale.
 *
 * Stays a plain `object` (not a Hilt repository) because
 * `attachBaseContext` cannot inject; [EaseMusicPlayerApplication.onCreate]
 * installs the bridge right after initializing it.
 */
object LanguageSetting {
    /** Installed by [EaseMusicPlayerApplication]; null only before app start. */
    @Volatile
    internal var bridge: Bridge? = null

    /** Locale actually wrapped into the current activity (updated in [attachBaseContext]). */
    @Volatile
    private var applied: AppLanguage = AppLanguage.SYSTEM

    private val _language = MutableStateFlow(AppLanguage.SYSTEM)

    /** Desired language per the backend; see class docs for the apply flow. */
    val language: StateFlow<AppLanguage> = _language.asStateFlow()

    /** The locale the current activity was created with; see class docs. */
    fun appliedLanguage(): AppLanguage = applied

    /**
     * Hydrate [language] from the backend. Fire-and-forget safe: failures
     * leave the flow at its previous value (the bridge logs the error).
     */
    suspend fun load() {
        val b = bridge ?: return
        val tag = b.call(BridgeMethods.Preference.GET_LANGUAGE)
            .unwrapOrNull()
            ?.payload
        _language.value = fromTag(tag)
    }

    /**
     * Persist [language] to the backend and publish it to [language] — the
     * activity-level collector reacts by recreating. No-op-ish on failure
     * (returns false, error logged through the bridge).
     */
    suspend fun save(language: AppLanguage): Boolean {
        val b = bridge ?: return false
        val ret = b.call(BridgeMethods.Preference.SAVE_LANGUAGE, language.tag)
        if (!ret.isSuccess) {
            ret.unwrapOrNull() // logs the failure detail
            return false
        }
        _language.value = language
        return true
    }

    /**
     * Wrap [newBase] with the currently-cached locale. No-op for
     * [AppLanguage.SYSTEM]. Records the applied value for the activity
     * collector (see class docs).
     */
    fun attachBaseContext(newBase: Context): Context {
        val current = _language.value
        applied = current
        val tag = current.tag ?: return newBase
        val locales = LocaleList.forLanguageTags(tag)
        val config = Configuration(newBase.resources.configuration)
        config.setLocale(locales[0])
        config.setLocales(locales)
        return newBase.createConfigurationContext(config)
    }

    private fun fromTag(tag: String?): AppLanguage =
        AppLanguage.entries.firstOrNull { it.tag == tag } ?: AppLanguage.SYSTEM
}
