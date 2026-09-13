package com.kutedev.easemusicplayer.singleton

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalConfiguration
import kotlinx.serialization.Serializable

/**
 * Manifest text that may be localized per locale tag — the Kotlin mirror of
 * the Rust-side `LocalizedString` normalization.
 *
 * Plugin manifests express a localizable field (`name`, `description`,
 * contribution `title` / `desc`) either as a plain string (the base text —
 * every pre-intl manifest) or as a tag→string map
 * (`{"en-US": "Play Counts", "zh-CN": "播放计数"}`); the Rust scan normalizes
 * both into this shape, so Kotlin never parses the union.
 */
@Serializable
data class LocalizedText(
    /** Fallback text (the plain string, or the map's `en-US` base). */
    val base: String,
    /** tag → text overrides (`"zh-CN"` → `"播放计数"`); empty for plain strings. */
    val locales: Map<String, String> = emptyMap(),
)

/**
 * Locale-tag resolution for [LocalizedText]: exact tag match (`zh-CN`) →
 * language-prefix match (locale `zh-TW` still picks the `zh-CN` entry; a
 * bare `zh` key covers any `zh-*` locale) → [LocalizedText.base].
 *
 * Locale selection is Kotlin-side on purpose: the Rust backend only knows
 * the `en` / `zh-CN` / null preference, while `SYSTEM` mode resolves from
 * the OS locale via the activity's wrapped configuration. Call
 * [resolve] at composition — the language-switch flow recreates the
 * activity, so composition-time reads always see the applied locale.
 */
fun resolveLocalizedText(text: LocalizedText, tag: String): String {
    if (text.locales.isNotEmpty()) {
        text.locales[tag]?.let { return it }
        val language = tag.substringBefore('-')
        if (language.isNotEmpty()) {
            text.locales[language]?.let { return it }
            for ((key, value) in text.locales) {
                if (key.substringBefore('-') == language) return value
            }
        }
    }
    return text.base
}

/** Resolve [text] against the current activity locale (see [resolveLocalizedText]). */
@Composable
fun LocalizedText.resolve(): String =
    resolveLocalizedText(this, LocalConfiguration.current.locales[0]?.toLanguageTag() ?: "")
