plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
}

android {
    namespace = "com.vex.connect.core.link"
    compileSdk = 35

    defaultConfig {
        minSdk = 28
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }

    // The Rust protocol library (libvex_link_ffi.so) and its generated Kotlin wrapper are built from crates/vex-link-ffi
    // by scripts/android-native.sh (the Android CI job runs it first). They are not committed.
    sourceSets {
        getByName("main") {
            java.srcDir("src/generated/kotlin")
            jniLibs.srcDir("src/generated/jniLibs")
        }
    }
}

dependencies {
    api(project(":core:domain"))

    implementation(libs.kotlinx.coroutines.android)
    // UniFFI's generated Kotlin code talks to the native library through JNA.
    implementation("net.java.dev.jna:jna:5.14.0@aar")

    testImplementation(libs.junit)
}
