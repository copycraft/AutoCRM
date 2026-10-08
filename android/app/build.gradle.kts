import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.ksp)
}

android {
    namespace = "hu.autotherm.autocrm"
    compileSdk = 35

    defaultConfig {
        applicationId = "hu.autotherm.autocrm"
        // API 26. Below that there is no adoptOpenJDK time API, no adaptive icons, and no
        // phone on this shop floor that old.
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // The schema JSON is committed. A future migration is reviewed against it, which is
        // the only way to be sure a schema change does not quietly drop queued photos.
        ksp { arg("room.schemaLocation", "$projectDir/schemas") }

        // Only the value the setup screen prefills. The address the app actually uses is
        // chosen on that screen and stored on the device (ServerStore), because one APK has
        // to follow a phone from the workshop Wi-Fi to the office to a customer's site.
        buildConfigField("String", "API_BASE_URL", "\"${apiBaseUrl()}\"")

        // Optional single-ABI build (-Pautocrm.abi=arm64-v8a). ML Kit's native libraries
        // ship for four ABIs; a sideloaded phone APK needs one, at about a third the size.
        (project.findProperty("autocrm.abi") as String?)?.let { ndk { abiFilters += it } }
    }

    // Release signing. `android/keystore.properties` (git-ignored; written by the root
    // build script) names the keystore. Without it the release APK is unsigned.
    val keystoreProps = Properties().apply {
        val file = rootProject.file("keystore.properties")
        if (file.exists()) file.inputStream().use(::load)
    }
    signingConfigs {
        if (keystoreProps.getProperty("storeFile") != null) {
            create("release") {
                storeFile = rootProject.file(keystoreProps.getProperty("storeFile"))
                storePassword = keystoreProps.getProperty("storePassword")
                keyAlias = keystoreProps.getProperty("keyAlias")
                keyPassword = keystoreProps.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        debug {
            // 10.0.2.2 is the host machine as seen from the Android emulator, which is the
            // right guess for a debug build. Overridable on the setup screen like any other.
            buildConfigField("String", "API_BASE_URL", "\"http://10.0.2.2:8080\"")
            applicationIdSuffix = ".debug"
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            signingConfigs.findByName("release")?.let { signingConfig = it }
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }
    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }
    testOptions {
        unitTests.isIncludeAndroidResources = true
    }
}

/** What the setup screen prefills in release builds; `local.properties` or the environment. */
fun apiBaseUrl(): String {
    val fromEnv = System.getenv("AUTOCRM_API_URL")
    if (!fromEnv.isNullOrBlank()) return fromEnv
    val props = Properties()
    val file = rootProject.file("local.properties")
    if (file.exists()) file.inputStream().use(props::load)
    return props.getProperty("autocrm.apiUrl") ?: "https://crm.autotherm.hu"
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.navigation.compose)

    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.graphics)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons)
    debugImplementation(libs.compose.ui.tooling)

    implementation(libs.room.runtime)
    implementation(libs.room.ktx)
    ksp(libs.room.compiler)

    implementation(libs.work.runtime.ktx)
    implementation(libs.datastore.preferences)

    implementation(libs.okhttp)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.coil.compose)

    // In-app camera for handover inspections only: the guided walkaround needs a live
    // view with instruction overlays and a tight shoot-review loop, which the system
    // camera intent cannot provide. Everything else still uses the system camera.
    implementation(libs.camera.camera2)
    implementation(libs.camera.lifecycle)
    implementation(libs.camera.view)
    implementation(libs.camera.video)
    implementation(libs.mlkit.text)
    implementation(libs.mlkit.barcode)
    implementation(libs.biometric)
    implementation(libs.glance.appwidget)

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.core)
    testImplementation(libs.okhttp.mockwebserver)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.room.testing)
}
