// C3：候选窗（Compose Desktop）—— 仓库镜像约定与 android/ 完全一致：
// 阿里云优先（本机 dl.google.com 被 DNS 劫持至 ~2KB/s），绝不写 google()。
pluginManagement {
    repositories {
        // 阿里云镜像优先：/repository/google 为 google maven 完整镜像，
        // gradle-plugin = Gradle Plugin Portal 镜像，public = Maven Central 镜像。
        maven { url = uri("https://maven.aliyun.com/repository/google") }
        maven { url = uri("https://maven.aliyun.com/repository/gradle-plugin") }
        maven { url = uri("https://maven.aliyun.com/repository/public") }
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.PREFER_SETTINGS)
    repositories {
        maven { url = uri("https://maven.aliyun.com/repository/google") }
        maven { url = uri("https://maven.aliyun.com/repository/gradle-plugin") }
        maven { url = uri("https://maven.aliyun.com/repository/public") }
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
