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
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
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

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.activity.compose)
    // Runs mission steps periodically in the background, surviving app
    // closure (and, via KEEP + reboot re-enqueue in MainActivity, reboots).
    implementation(libs.androidx.work.runtime.ktx)
    implementation(libs.androidx.core.ktx)
    // UniFFI's Kotlin bindings load the native library through JNA.
    implementation("${libs.jna.get()}@aar")

    testImplementation(libs.junit)
    testImplementation(libs.jna)
}
