plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "com.vex.connect"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.vex.connect"
        minSdk = 28
        // Targets API 35 (Android 15, the OxygenOS 15 release on the test phone). Staying below the API level that
        // introduces the separate local-network permission keeps the permission list to the three the app really needs.
        targetSdk = 35
        versionCode = 2
        versionName = "0.2.0-stage2"
    }

    // Fixed debug signing key (see docs/VEX_CONNECT_ANDROID.md). CI passes it through environment variables
    // that come from GitHub Secrets. Without them (a local build) the standard Android debug key is used.
    val debugKeystore = System.getenv("VEX_DEBUG_KEYSTORE_FILE")
    signingConfigs {
        getByName("debug") {
            if (!debugKeystore.isNullOrBlank()) {
                storeFile = file(debugKeystore)
                storePassword = System.getenv("VEX_DEBUG_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("VEX_DEBUG_KEY_ALIAS")
                keyPassword = System.getenv("VEX_DEBUG_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        debug {
            signingConfig = signingConfigs.getByName("debug")
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }
}

dependencies {
    implementation(project(":core:designsystem"))
    implementation(project(":core:domain"))
    implementation(project(":core:data"))
    implementation(project(":core:link"))

    // QR scanning: CameraX for the preview, ML Kit (bundled model, no Google Play services needed) to read the code.
    implementation("androidx.camera:camera-core:1.4.1")
    implementation("androidx.camera:camera-camera2:1.4.1")
    implementation("androidx.camera:camera-lifecycle:1.4.1")
    implementation("androidx.camera:camera-view:1.4.1")
    implementation("com.google.mlkit:barcode-scanning:17.3.0")

    val bom = platform(libs.androidx.compose.bom)
    implementation(bom)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.core.splashscreen)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.navigation.compose)
    implementation(libs.androidx.compose.ui.tooling.preview)
    debugImplementation(libs.androidx.compose.ui.tooling)

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
}
