// 検索エンジン(Rust クレート `unfydqry`)の Kotlin バインディングを格納する純 JVM モジュール。
// iOS 側の Sources/UnifiedQuery/ (SwiftPM ライブラリターゲット UnifiedQuery) と対をなす。
//
// `:app` はこのモジュールに依存し、Android 上では :app の jniLibs/ に置かれる
// libunfydqry.so 経由で JNA がロードする。
// このモジュール単体の JVM テストでは、core/target/aarch64-apple-darwin/release/
// にビルドされた libunfydqry.dylib を `jna.library.path` で参照する。
plugins {
    id("org.jetbrains.kotlin.jvm")
}

// リポジトリは settings.gradle.kts の dependencyResolutionManagement から継承する。

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
    // 生成バインディングが要求する唯一のコンパイル時依存。
    // 実行時の JNA は Android 上では :app が `jna:5.14.0@aar` で持ち込み(libjnidispatch.so 同梱)、
    // JVM 単体テストでは下記の testImplementation で持ち込む。
    compileOnly("net.java.dev.jna:jna:5.14.0")

    testImplementation("net.java.dev.jna:jna:5.14.0")
    testImplementation(platform("org.junit:junit-bom:5.11.3"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
    // Spec JSON ローダ用。Kotlin の data class へ素直にデコードできる Jackson の
    // Kotlin モジュールを使う(serialization プラグインの追加なしで済む)。
    testImplementation("com.fasterxml.jackson.module:jackson-module-kotlin:2.18.2")
}

tasks.test {
    useJUnitPlatform()
    // macOS arm64 ホストで `cargo build --release --target aarch64-apple-darwin` した
    // dylib を JNA から拾えるようにする。
    val dylibDir = rootProject.layout.projectDirectory.dir(
        "../../core/target/aarch64-apple-darwin/release"
    ).asFile.absolutePath
    systemProperty("jna.library.path", dylibDir)

    // 共有 spec ディレクトリの絶対パスを system property で渡す
    // (Swift / Rust 側と同じ JSON を参照させるため)。
    val specDir = rootProject.layout.projectDirectory.dir("../../spec").asFile.absolutePath
    systemProperty("unfydqry.spec.dir", specDir)

    testLogging {
        events("passed", "skipped", "failed")
        showStandardStreams = false
    }
}
