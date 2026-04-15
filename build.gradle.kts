plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.multiplatform) apply false
    alias(libs.plugins.compose.compiler) apply false
    alias(libs.plugins.jetbrains.compose) apply false
    kotlin("plugin.serialization") version libs.versions.kotlin apply false
    id("com.google.devtools.ksp") version "2.0.21-1.0.27" apply false
}
