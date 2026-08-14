plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "io.opi.input"
    compileSdk = 34
    ndkVersion = "27.0.12077973"

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "io.opi.input"
        minSdk = 21
        targetSdk = 34
        versionCode = 1
        versionName = "1.0.0"
        ndk {
            // 与 rust_builder cargokit targets 对齐（plugin.gradle 固定三 ABI）。
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    buildTypes {
        release {
            // 本环境无 flutter_embedding_release 工件，只构建 debug；
            // release 暂用 debug 签名保证可打包。
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

dependencies {
    // rust_builder（cargokit 独立版，includeBuild）：提供 libopi_ffi.so（jniLibs 进 AAR）。
    // includeBuild 子项目按坐标引用（rust_builder/android/build.gradle: group/version）。
    implementation("com.flutter_rust_bridge.rust_lib_app:android:1.0")
    // Compose：BOM 2024.09.00 兼容 compileSdk 34（只约束 androidx.compose.*）；
    // activity-compose 属 androidx.activity 组（不在 BOM 内），需显式版本。
    implementation(platform("androidx.compose:compose-bom:2024.09.00"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.9.2")
}
