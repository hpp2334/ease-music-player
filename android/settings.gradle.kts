pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "Ease Music Player"
include(":app")

// cantode's Kotlin facade lives with the engine (cantode/kotlin), not in
// the Android source tree. One Gradle build, two homes.
include(":cantode-engine")
project(":cantode-engine").projectDir = file("../cantode/kotlin")
