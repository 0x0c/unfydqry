// Pure-JVM module that hosts the Kotlin binding for the search engine (Rust crate `unfydqry`).
// Mirrors the iOS-side Sources/UnifiedQuery/ (the SwiftPM library target UnifiedQuery).
//
// `:app` depends on this module. On Android, JNA loads the native library from
// libunfydqry.so placed under :app's jniLibs/.
// For this module's JVM unit tests, JNA loads libunfydqry.dylib built into
// core/target/aarch64-apple-darwin/release/ (passed via `jna.library.path`).
plugins {
    id("org.jetbrains.kotlin.jvm")
}

// Repositories are inherited from dependencyResolutionManagement in settings.gradle.kts.

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    // The only compile-time dependency required by the generated binding.
    // At runtime, JNA is pulled in by :app's `jna:5.14.0@aar` on Android (which bundles
    // libjnidispatch.so) and by the testImplementation entry below for JVM unit tests.
    compileOnly("net.java.dev.jna:jna:5.14.0")

    testImplementation("net.java.dev.jna:jna:5.14.0")
    testImplementation(platform("org.junit:junit-bom:5.14.4"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
    // Used by the spec JSON loader. Jackson's Kotlin module decodes straight into
    // data classes without having to pull in the kotlinx.serialization plugin.
    testImplementation("com.fasterxml.jackson.module:jackson-module-kotlin:2.18.2")
}

tasks.test {
    useJUnitPlatform()
    // Make the dylib produced by `cargo build --release --target aarch64-apple-darwin`
    // on a macOS arm64 host discoverable by JNA.
    val dylibDir = rootProject.layout.projectDirectory.dir(
        "../../core/target/aarch64-apple-darwin/release"
    ).asFile.absolutePath
    systemProperty("jna.library.path", dylibDir)

    // Pass the absolute path of the shared spec directory through a system property
    // so the tests read the same JSON files as the Swift and Rust runners.
    val specDir = rootProject.layout.projectDirectory.dir("../../spec").asFile.absolutePath
    systemProperty("unfydqry.spec.dir", specDir)

    testLogging {
        events("passed", "skipped", "failed")
        showStandardStreams = false
    }
}
