plugins {
    // AGP 9.0+ provides built-in Kotlin support, so the standalone
    // org.jetbrains.kotlin.android plugin is no longer applied here.
    id("com.android.library")
}

android {
    namespace = "unfydqry.flutter"
    compileSdk = 36

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig { minSdk = 24 }

    // AGP 9 (built-in Kotlin) source-set DSL: configure "main" via sourceSets { named(...) }.
    // The old `sourceSets["main"].kotlin` accessor throws a cast error under AGP 9.
    sourceSets {
        named("main") {
            // Re-use the generated UniFFI Kotlin binding from the Android module.
            kotlin.srcDirs(
                "src/main/kotlin",
                "../../android/sample/unifiedquery/src/main/kotlin",
            )
            // Re-use the pre-built .so files from the Android module.
            jniLibs.srcDirs("../../android/jniLibs")
        }
    }
}

// AGP 9 dropped the android `kotlinOptions {}` DSL; configure the built-in
// Kotlin compiler through the top-level `kotlin { compilerOptions { } }` block.
kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    // JNA is required by the UniFFI generated binding at both compile- and run-time.
    compileOnly("net.java.dev.jna:jna:5.19.1")
    implementation("net.java.dev.jna:jna:5.19.1@aar")
}
