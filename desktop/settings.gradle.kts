// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// C3：候选窗（Compose Desktop）—— 仓库镜像约定与 android/ 完全一致：
// 阿里云优先（本机 dl.google.com 被 DNS 劫持至 ~2KB/s），绝不写 google()。
pluginManagement {
    repositories {
        // 阿里云镜像优先：/repository/google 为 google maven 完整镜像，
        // gradle-plugin = Gradle Plugin Portal 镜像，public = Maven Central 镜像。
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
    // Compose Multiplatform 插件（Maven Central 镜像可解析，2026-08 最新稳定版 1.11.x）。
    id("org.jetbrains.compose") version "1.11.1" apply false
    // Kotlin 版本与 android/ 对齐（本机 ~/.gradle 缓存已有 2.3.20 全套插件）。
    id("org.jetbrains.kotlin.jvm") version "2.3.20" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.3.20" apply false
}

// 根项目即应用本身（settings.gradle.kts 与 build.gradle.kts 同目录，无子模块）；
// 命名为 "desktop" 使 `./gradlew :desktop:package` 与计划命令一致。
rootProject.name = "desktop"
