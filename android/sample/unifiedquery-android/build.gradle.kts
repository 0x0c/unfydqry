// Android Library wrapper that bundles the generated Kotlin binding together with
// the prebuilt libunfydqry.so for each Android ABI, distributed as an AAR via
// Maven Central. The Kotlin source is shared with the pure-JVM `:unifiedquery`
// module (which exists for `gradle :unifiedquery:test`).
import com.vanniktech.maven.publish.AndroidSingleVariantLibrary

plugins {
    // AGP 9.0+ provides built-in Kotlin support, so the standalone
    // org.jetbrains.kotlin.android plugin is no longer applied here.
    id("com.android.library")
    id("com.vanniktech.maven.publish")
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

android {
    namespace = "com.unfydqry.unifiedquery"
    compileSdk = 36
    // Match the NDK installed at ndk.dir (r29) so bundled native .so files are stripped.
    ndkVersion = "29.0.14206865"
    defaultConfig {
        minSdk = 29
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // AGP 9 (built-in Kotlin) source-set DSL: configure "main" via sourceSets { named(...) }.
    sourceSets {
        named("main") {
            kotlin.srcDir("../unifiedquery/src/main/kotlin")
            // cargo-ndk writes libunfydqry.so/<ABI>/libunfydqry.so under android/jniLibs/.
            // From this module: android/sample/unifiedquery-android/ → ../../jniLibs.
            jniLibs.srcDir("../../jniLibs")
        }
    }
}

dependencies {
    // JNA is required at runtime by the generated binding; the @aar variant
    // additionally ships libjnidispatch.so for every Android ABI, so consumers
    // get a working setup with a single dependency.
    api("net.java.dev.jna:jna:5.14.0@aar")
}

mavenPublishing {
    // vanniktech 0.30+ publishes to the Central Portal by default; the SonatypeHost
    // argument was removed, so only automaticRelease is passed now.
    publishToMavenCentral(automaticRelease = true)
    signAllPublications()

    configure(AndroidSingleVariantLibrary(variant = "release", sourcesJar = true, publishJavadocJar = false))

    coordinates(
        groupId = providers.gradleProperty("GROUP").get(),
        artifactId = providers.gradleProperty("POM_ARTIFACT_ID").get(),
        version = providers.gradleProperty("VERSION_NAME").get(),
    )

    pom {
        name.set(providers.gradleProperty("POM_NAME"))
        description.set(providers.gradleProperty("POM_DESCRIPTION"))
        url.set(providers.gradleProperty("POM_URL"))
        licenses {
            license {
                name.set(providers.gradleProperty("POM_LICENSE_NAME"))
                url.set(providers.gradleProperty("POM_LICENSE_URL"))
                distribution.set("repo")
            }
        }
        developers {
            developer {
                id.set(providers.gradleProperty("POM_DEVELOPER_ID"))
                name.set(providers.gradleProperty("POM_DEVELOPER_NAME"))
                url.set(providers.gradleProperty("POM_DEVELOPER_URL"))
            }
        }
        scm {
            url.set(providers.gradleProperty("POM_SCM_URL"))
            connection.set(providers.gradleProperty("POM_SCM_CONNECTION"))
            developerConnection.set(providers.gradleProperty("POM_SCM_DEV_CONNECTION"))
        }
    }
}
