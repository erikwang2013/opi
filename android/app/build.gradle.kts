// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "io.opi.input"
    compileSdk = 34
    ndkVersion = "27.0.12077973"

    // 共享源码：小欧的 Compose 绘制只有一份（shared/pet），同时供本模块与
    // desktop/ 的 Windows 候选窗使用 —— 两处重复一份几何必然漂移。
    sourceSets.getByName("main").kotlin.srcDir("../../shared/pet")

    // .opid 词库不压缩存储。压缩后 Assets.openFd() 会抛 IOException，
    // EngineLoader.assetLength() 只能返回 null → needsCopy 恒为 true，
    // 于是每次建 IME 视图、每次开设置页都在主线程重拷 3.3MB
    // （luna 1.24MB + trad 2.08MB），尺寸校验的幂等优化被完全废掉。
    // 代价：APK 增大约 1.5MB（实测 3.3MB 压缩到 1.76MB）—— 换输入法启动延迟，值。
    androidResources {
        noCompress += setOf("opid")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "io.opi.input"
        minSdk = 21
        targetSdk = 34
        // 与发布 tag 对齐（此前 9 个 tag 期间 versionCode 一直是 1、versionName 一直是
        // "1.0.0"：APK 既无法被识别，也无法覆盖安装升级）。
        versionCode = 14
        versionName = "1.3.3"
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
    testImplementation("junit:junit:4.13.2")
}
