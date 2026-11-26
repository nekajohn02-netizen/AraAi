import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

val rustJniLibs = layout.buildDirectory.dir("rust/jniLibs")
val rustKotlin = layout.buildDirectory.dir("rust/kotlin")
val rustHostLib = layout.buildDirectory.dir("rust/host")
val araNdkVersion = "29.0.14206865"

// SDK location: local.properties `sdk.dir`, else ANDROID_HOME.
val androidSdkDir: String = run {
    val lp = rootProject.file("local.properties")
    val fromLocal = if (lp.exists()) {
        Properties().apply { lp.inputStream().use { load(it) } }.getProperty("sdk.dir")
    } else null
    fromLocal ?: System.getenv("ANDROID_HOME")
        ?: error("Android SDK not found: set sdk.dir in local.properties or ANDROID_HOME")
}

android {
    namespace = "ai.ara.app"
    compileSdk = 37
    ndkVersion = araNdkVersion

    defaultConfig {
        applicationId = "ai.ara.app"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        // armeabi-v7a: older/budget 32-bit-only devices (e.g. Samsung
        // Galaxy Tab A6, 2016) have no 64-bit ABI at all — without this,
        // the app installs but crashes immediately on launch because the
        // native core library can't be loaded (confirmed 2026-11-26).
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures { compose = true }

    sourceSets.getByName("main") {
        jniLibs.directories.add(rustJniLibs.get().asFile.path)
        kotlin.directories.add(rustKotlin.get().asFile.path)
    }
    testOptions.unitTests.all {
        it.systemProperty("jna.library.path", rustHostLib.get().asFile.path)
    }
}

kotlin {
    compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) }
}

// Builds the Rust core (all ABIs + host) and generates UniFFI Kotlin bindings.
val buildRustCore = tasks.register<Exec>("buildRustCore") {
    val script = rootProject.file("../scripts/build-core.sh")
    inputs.dir(rootProject.file("../core/crates"))
    inputs.file(rootProject.file("../core/Cargo.lock"))
    inputs.file(script)
    outputs.dirs(rustJniLibs, rustKotlin, rustHostLib)
    environment("ANDROID_NDK_HOME", "$androidSdkDir/ndk/$araNdkVersion")
    commandLine(
        script.path,
        rustJniLibs.get().asFile.path,
        rustKotlin.get().asFile.path,
        rustHostLib.get().asFile.path,
        "release",
    )
}
tasks.named("preBuild") { dependsOn(buildRustCore) }

// Regenerates the launcher/notification/splash PNGs in res/ from the master
// logo (android/branding/ara_logo_master.png). NOT part of preBuild on
// purpose: assembleDebug/assembleRelease must keep working on a machine
// without Python/Pillow, using the PNGs this task already committed to
// res/. Run manually after replacing the master logo:
//     ./gradlew regenerateBrandIcons
val regenerateBrandIcons = tasks.register<Exec>("regenerateBrandIcons") {
    group = "ara branding"
    description = "Regenerates mipmap/drawable icon assets from android/branding/ara_logo_master.png (requires python3 + Pillow)."
    val script = rootProject.file("branding/generate_icons.py")
    inputs.file(script)
    inputs.file(rootProject.file("branding/ara_logo_master.png"))
    commandLine("python3", script.path)
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.animation)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.activity.compose)
    // Runs mission steps periodically in the background, surviving app
    // closure (and, via KEEP + reboot re-enqueue in MainActivity, reboots).
    implementation(libs.androidx.work.runtime.ktx)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.core.splashscreen)
    // UniFFI's Kotlin bindings load the native library through JNA.
    implementation("${libs.jna.get()}@aar")

    testImplementation(libs.junit)
    testImplementation(libs.jna)
}
