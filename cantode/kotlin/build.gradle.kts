plugins {
    // kotlin-jvm carries no version (KGP is already on the build
    // classpath via :app); serialization resolves with the same version
    // :app uses.
    id("org.jetbrains.kotlin.jvm")
    kotlin("plugin.serialization") version "2.0.0"
}

// Pure-JVM engine facade — no Android dependencies. Consumed by the
// Android app (`:app`) as a library; keep the toolchain aligned with the
// app's compileOptions (Java 21).

java {
    sourceCompatibility = JavaVersion.VERSION_21
    targetCompatibility = JavaVersion.VERSION_21
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21)
    }
}

dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.3")
}
