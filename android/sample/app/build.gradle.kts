plugins {
    // AGP 9.0+ provides built-in Kotlin support, so the standalone
    // org.jetbrains.kotlin.android plugin is no longer applied here.
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.unfydqry.searchsample"
    compileSdk = 36
    // Match the NDK installed at ndk.dir (r29) so native .so files are stripped
    // instead of being packaged unstripped (AGP's default ndkVersion differs).
    ndkVersion = "29.0.14206865"

    defaultConfig {
        applicationId = "com.unfydqry.searchsample"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.1"
    }

    buildFeatures { compose = true }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    // The binding (uniffi.unfydqry.*) is brought in via the :unifiedquery module.
    // Symmetric to depending on the SwiftPM library target (UnifiedQuery) on iOS.
    implementation(project(":unifiedquery"))

    implementation("androidx.activity:activity-compose:1.13.0")
    implementation(platform("androidx.compose:compose-bom:2026.05.01"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    // JNA's native dispatch part (libjnidispatch.so) is only delivered via the AAR.
    // :unifiedquery is distributed as a jar, so on Android the :app side must add this AAR.
    implementation("net.java.dev.jna:jna:5.14.0@aar")
}
