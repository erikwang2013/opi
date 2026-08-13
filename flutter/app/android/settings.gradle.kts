pluginManagement {
    val flutterSdkPath =
        run {
            val properties = java.util.Properties()
            file("local.properties").inputStream().use { properties.load(it) }
            val flutterSdkPath = properties.getProperty("flutter.sdk")
            require(flutterSdkPath != null) { "flutter.sdk not set in local.properties" }
            flutterSdkPath
        }

    includeBuild("$flutterSdkPath/packages/flutter_tools/gradle")

    repositories {
        // 阿里云镜像优先：本机 dl.google.com 被 DNS 劫持至 ~2KB/s 慢速镜像，
        // 阿里云 /repository/google 为 google maven 完整镜像（~4MB/s）。
        maven { url = uri("https://maven.aliyun.com/repository/google") }
        maven { url = uri("https://maven.aliyun.com/repository/gradle-plugin") }
        maven { url = uri("https://maven.aliyun.com/repository/public") }
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    // 阿里云镜像优先（同 pluginManagement 注释：dl.google.com 被 DNS 劫持），
    // 覆盖 runtime 依赖（androidx、io.flutter:flutter_embedding 等）。
    repositoriesMode.set(RepositoriesMode.PREFER_SETTINGS)
    repositories {
        // 本地引擎仓库优先：io.flutter 引擎工件（embedding + 三 ABI libflutter.so）
        // 仅发布于 dl.google.com（被 DNS 劫持），镜像均无 → 从 SDK 缓存构造本地 m2。
        maven { url = uri("file:///home/erik/opi_local_m2") }
        maven { url = uri("https://maven.aliyun.com/repository/google") }
        maven { url = uri("https://maven.aliyun.com/repository/public") }
        // 原始 google()（dl.google.com）已移除：动态版本（如 androidx.test:runner:1.2+）
        // 会查询所有仓库的 maven-metadata.xml，dl.google.com 连接超时会导致构建失败；
        // aliyun google 镜像为完整镜像（已验证 androidx.test metadata 200）。
        mavenCentral()
    }
}

plugins {
    id("dev.flutter.flutter-plugin-loader") version "1.0.0"
    id("com.android.application") version "9.0.1" apply false
    id("org.jetbrains.kotlin.android") version "2.3.20" apply false
}

include(":app")
