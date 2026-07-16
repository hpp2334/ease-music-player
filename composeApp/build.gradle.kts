import org.jetbrains.compose.desktop.application.dsl.TargetFormat
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.io.FileInputStream
import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.compose.compiler)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.kotlin.serialization)
}

val appVersion = "0.3.0"

val osName = System.getProperty("os.name").lowercase()
val osArch = System.getProperty("os.arch").lowercase()
val javafxClassifier = when {
    osName.contains("mac") && osArch == "aarch64" -> "mac-aarch64"
    osName.contains("mac") -> "mac"
    osName.contains("win") -> "win"
    else -> "linux"
}

val keystoreProperties = Properties()
val keystorePropertiesFile = rootProject.file("key.properties")
if (keystorePropertiesFile.exists()) {
    keystoreProperties.load(FileInputStream(keystorePropertiesFile))
}

kotlin {
    androidTarget {
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_21)
        }
    }
    jvm("desktop")

    sourceSets {
        val commonMain by getting {
            dependencies {
                implementation(compose.runtime)
                implementation(compose.foundation)
                implementation(compose.material3)
                implementation(compose.ui)
                implementation(compose.uiTooling)
                implementation(compose.components.resources)
                implementation(libs.kotlinx.coroutines.core)
                implementation(libs.kotlinx.serialization.json)
                implementation(libs.kotlinx.collections.immutable)
                implementation(libs.navigation.compose)
                implementation(libs.lifecycle.viewmodel.compose)
                implementation(libs.lifecycle.runtime.compose)
                implementation(libs.koin.core)
                implementation(libs.koin.compose)
                implementation(libs.koin.compose.viewmodel)
                implementation(libs.reorderable)
            }
        }
        // Shared JVM source set for code that uses JNA/UniFFI (Android + Desktop)
        val jvmShared by creating {
            dependsOn(commonMain)
            dependencies {
                implementation(libs.jna)
            }
        }
        val androidMain by getting {
            dependsOn(jvmShared)
            dependencies {
                implementation(libs.androidx.core.ktx)
                implementation(libs.androidx.activity.compose)
                implementation(libs.androidx.media3.exoplayer)
                implementation(libs.androidx.media3.exoplayer.dash)
                implementation(libs.androidx.media3.session)
                implementation(libs.androidx.media3.datasource)
            }
        }
        val desktopMain by getting {
            dependsOn(jvmShared)
            dependencies {
                implementation(compose.desktop.currentOs)
                implementation(libs.kotlinx.coroutines.swing)
                val jfx = libs.versions.javafx.get()
                implementation("org.openjfx:javafx-media:$jfx:$javafxClassifier")
                implementation("org.openjfx:javafx-graphics:$jfx:$javafxClassifier")
                implementation("org.openjfx:javafx-base:$jfx:$javafxClassifier")
            }
        }
    }
}

android {
    namespace = "com.kutedev.easemusicplayer"
    compileSdk = 35

    lint {
        abortOnError = false
    }

    defaultConfig {
        applicationId = "com.kutedev.easemusicplayer"
        minSdk = 29
        targetSdk = 34
        versionCode = 1
        versionName = appVersion

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables {
            useSupportLibrary = true
        }
    }

    signingConfigs {
        create("release") {
            keyAlias = keystoreProperties["keyAlias"].toString()
            keyPassword = keystoreProperties["keyPassword"].toString()
            storeFile = if (keystoreProperties["storeFile"] != null) {
                rootProject.file(keystoreProperties["storeFile"].toString())
            } else {
                null
            }
            storePassword = keystoreProperties["storePassword"].toString()
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            signingConfig = signingConfigs.getByName("release")
            multiDexEnabled = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            ndk {
                debugSymbolLevel = "SYMBOL_TABLE"
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
        jniLibs {
            useLegacyPackaging = true
        }
    }

    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a")
            isUniversalApk = false
        }
    }

    sourceSets["main"].apply {
        manifest.srcFile("src/androidMain/AndroidManifest.xml")
        res.srcDirs("src/androidMain/res")
    }
}

compose.desktop {
    application {
        mainClass = "com.kutedev.easemusicplayer.MainKt"

        nativeDistributions {
            targetFormats(TargetFormat.Dmg, TargetFormat.Deb, TargetFormat.Msi)
            packageName = "ease-music-player"
            packageVersion = appVersion
            macOS {
                dmgPackageVersion = "1.0.0"
                packageBuildVersion = "1.0.0"
            }
        }
    }
}
