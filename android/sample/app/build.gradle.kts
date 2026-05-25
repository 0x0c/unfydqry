plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.unimose.searchsample"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.unimose.searchsample"
        minSdk = 29
        targetSdk = 34
        versionCode = 1
        versionName = "0.1"
    }

    buildFeatures { compose = true }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions { jvmTarget = "17" }

    packaging {
        // .so は jniLibs から拾う
    }
}

dependencies {
    // バインディング(uniffi.search_core.*)は :searchcore モジュール経由で取り込む。
    // iOS の SwiftPM ライブラリターゲットを参照するのと対称。
    implementation(project(":searchcore"))

    implementation("androidx.activity:activity-compose:1.9.3")
    implementation(platform("androidx.compose:compose-bom:2024.10.01"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    // JNA のネイティブ・ディスパッチ部(libjnidispatch.so)は AAR からしか入らない。
    // :searchcore は jar 配布なので、Android 上では app 側でこの AAR を足す必要がある。
    implementation("net.java.dev.jna:jna:5.14.0@aar")
}
