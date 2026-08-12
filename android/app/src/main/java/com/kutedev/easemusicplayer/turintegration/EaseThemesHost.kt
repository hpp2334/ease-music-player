package com.kutedev.easemusicplayer.turintegration

/**
 * JNI upcall target for the Rust `ease:themes` host module.
 *
 * Kotlin is the source of truth for the app theme — the Compose
 * `MaterialTheme.colorScheme` (resolved in `EaseMusicPlayerTheme`) is pushed
 * here via [setTheme] on every recomposition. The Rust `ease:themes` bridge
 * (in `rust-libs/.../plugin_runtime/themes_bridge.rs`) reads it back through
 * [getColor] / [isDark] via JNI static-method upcalls, so plugin JS can call
 * `ease:themes.color("primary")` / `ease:themes.isDark()` and inherit the
 * host app's visual style.
 *
 * All fields are `@Volatile` — set from the Compose main thread, read from
 * the engine's JNI upcall thread.
 */
object EaseThemesHost {
    @Volatile
    private var colors: Map<String, String> = emptyMap()

    @Volatile
    private var dark: Boolean = false

    /**
     * Push the current theme snapshot. Called from
     * `EaseMusicPlayerTheme`'s `SideEffect`. [colors] maps role names
     * (`"primary"`, `"surface"`, …) to `"#AARRGGBB"` hex strings;
     * [isDark] is the resolved dark/light flag.
     */
    @JvmStatic
    fun setTheme(isDark: Boolean, colors: Map<String, String>) {
        this.dark = isDark
        this.colors = colors
    }

    /** `ease:themes.color(name)` upcall entry. Returns `"#AARRGGBB"` or `""`. */
    @JvmStatic
    fun getColor(name: String): String = colors[name] ?: ""

    /** `ease:themes.isDark()` upcall entry. */
    @JvmStatic
    fun isDark(): Boolean = dark
}
