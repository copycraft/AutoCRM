// The Android client lives in this repository, not a separate one: it speaks a contract
// generated from the same OpenAPI document as the web client, and a change to that contract
// has to be able to break both in one commit.
pluginManagement {
    repositories {
        google()
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

rootProject.name = "AutoCRM"
include(":app")
