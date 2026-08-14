pluginManagement {
    repositories {
        // 阿里云镜像优先：本机 dl.google.com 被 DNS 劫持至 ~2KB/s 慢速镜像，
        // 阿里云 /repository/google 为 google maven 完整镜像（~4MB/s）。
        maven { url = uri("https://maven.aliyun.com/repository/google") }
        maven { url = uri("https://maven.aliyun.com/repository/gradle-plugin") }
        maven { url = uri("https://maven.aliyun.com/repository/public") }
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
    }
}

plugins {
    id("com.android.application") version "9.0.1" apply false
    id("org.jetbrains.kotlin.android") version "2.3.20" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.3.20" apply false
}

include(":app")

// cargokit 独立版（flutter_rust_bridge）：编译 crates/opi-ffi 为三 ABI so。
// android/rust_builder/android/build.gradle 内配置 manifestDir=../../../../crates/opi-ffi、
// libname=opi_ffi；app 通过 implementation(project(":android")) 引用。
includeBuild("rust_builder")
