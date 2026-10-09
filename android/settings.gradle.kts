// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

pluginManagement {
    repositories {
        // 阿里云镜像优先：本机 dl.google.com 被 DNS 劫持至 ~2KB/s 慢速镜像，
        // 阿里云 /repository/google 为 google maven 完整镜像（~4MB/s）。
        maven { url = uri("https://maven.aliyun.com/repository/google") }
        maven { url = uri("https://maven.aliyun.com/repository/gradle-plugin") }
        maven { url = uri("https://maven.aliyun.com/repository/public") }
        // ⚠️ 兜底：**只在阿里云不可达/半途失败时命中**（2026-10-09：CI 一小时里被阿里云
        // 坑了两次 —— 插件解析 404 一次、构建中途 GET 失败一次，都是瞬时故障）。Gradle
        // 按序尝试，本机（阿里云快）走不到这里。
        // **仍绝不写 google()**（理由同上，那是 dl.google.com 的拦截）；mavenCentral 是
        // 另一个主机（repo.maven.apache.org），与那条禁令无关。注意：AGP 本体只在
        // Google Maven 上，若哪天连它也被镜像拖死，这条兜底救不了 —— 到时再议。
        mavenCentral()
    }
}

dependencyResolutionManagement {
    // 阿里云镜像优先（同 pluginManagement 注释：dl.google.com 被 DNS 劫持）。
    // 绝不写 google()：动态版本查询会连 dl.google.com 导致构建失败/超慢。
    repositoriesMode.set(RepositoriesMode.PREFER_SETTINGS)
    repositories {
        maven { url = uri("https://maven.aliyun.com/repository/google") }
        maven { url = uri("https://maven.aliyun.com/repository/gradle-plugin") }
        maven { url = uri("https://maven.aliyun.com/repository/public") }
        // ⚠️ 兜底：**只在阿里云不可达/半途失败时命中**（2026-10-09：CI 一小时里被阿里云
        // 坑了两次 —— 插件解析 404 一次、构建中途 GET 失败一次，都是瞬时故障）。Gradle
        // 按序尝试，本机（阿里云快）走不到这里。
        // **仍绝不写 google()**（理由同上，那是 dl.google.com 的拦截）；mavenCentral 是
        // 另一个主机（repo.maven.apache.org），与那条禁令无关。注意：AGP 本体只在
        // Google Maven 上，若哪天连它也被镜像拖死，这条兜底救不了 —— 到时再议。
        mavenCentral()
    }
}

plugins {
    id("com.android.application") version "9.0.1" apply false
    id("org.jetbrains.kotlin.android") version "2.3.20" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.3.20" apply false
}

include(":app")

// cargokit 独立版（flutter_rust_bridge）：编译 crates/opi-ffi 为三 ABI so。
// includeBuild 子项目按坐标（group:version 见 rust_builder/android/build.gradle）被依赖替换；
// app/build.gradle.kts 通过 implementation("com.flutter_rust_bridge.rust_lib_app:android:1.0") 引用。
includeBuild("rust_builder")
