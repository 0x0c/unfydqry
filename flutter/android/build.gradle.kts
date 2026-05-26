plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "unfydqry.flutter"
    compileSdk = 34

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions { jvmTarget = "17" }
    defaultConfig { minSdk = 24 }

    sourceSets["main"].apply {
        // Re-use the generated UniFFI Kotlin binding from the Android module.
        kotlin.srcDirs(
            "src/main/kotlin",
            "../../android/sample/unifiedquery/src/main/kotlin",
        )
        // Re-use the pre-built .so files from the Android module.
        jniLibs.srcDirs("../../android/jniLibs")
    }
}

dependencies {
    // JNA is required by the UniFFI generated binding at both compile- and run-time.
    compileOnly("net.java.dev.jna:jna:5.14.0")
    implementation("net.java.dev.jna:jna:5.14.0@aar")
}
