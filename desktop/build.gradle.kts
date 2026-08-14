// C3：候选窗（Compose Desktop）构建配置。
// 仓库镜像在 settings.gradle.kts（与 android/ 同约定，仅 aliyun，无 google()）。
plugins {
    kotlin("jvm")
    id("org.jetbrains.compose")
    id("org.jetbrains.kotlin.plugin.compose")
}

dependencies {
    // Compose Desktop 运行时（skiko 原生库随发布包分发）。
    // 注意：compose.material3 在 CMP 1.11 已弃用（error 级）且 material3 独立版本化
    // （1.9.0 stable 与 1.11 runtime 兼容性存疑）→ 候选窗 UI 用 foundation 纯自绘。
    implementation(compose.desktop.currentOs)
    // C3：named pipe 服务器端（JNA 读 kernel32：CreateNamedPipeW/ConnectNamedPipe/
    // ReadFile/WriteFile）。5.6.0 为本机 ~/.gradle 缓存版本（离线可解析）。
    implementation("net.java.dev.jna:jna:5.6.0")
    implementation("net.java.dev.jna:jna-platform:5.6.0")
}

kotlin {
    compilerOptions {
        // Compose Desktop 要求 JVM target >= 11；不配置 toolchain（避免离线下载 JDK）。
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_11)
    }
}

java {
    // 与 Kotlin 的 JVM_11 对齐（本机 JDK 18 运行 Gradle，java 默认 target 18 会冲突）。
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}

compose.desktop {
    application {
        mainClass = "io.opi.candidate.MainKt"

        nativeDistributions {
            // 构建验证用 Deb（Linux 主机，dpkg-deb 本机可用）；CMP 1.11 已移除
            // Zip 格式（枚举仅 AppImage/Deb/Rpm/Dmg/Pkg/Exe/Msi）。
            // Windows .msi 为验收阶段（Windows 主机上构建）。
            targetFormats(org.jetbrains.compose.desktop.application.dsl.TargetFormat.Deb)
            packageName = "opi-candidates"
            packageVersion = "0.1.0"
            description = "OPI 拼音输入法候选窗（TSF 插件经 named pipe 通信）"
            vendor = "OPI"
        }
    }
}
