// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

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
    // 本模块此前无 test 源集（`./gradlew test` → NO-SOURCE）。JUnit 4 是唯一
    // 本机 ~/.gradle 缓存里齐备（junit+hamcrest）的测试框架，离线可解析；
    // kotlin-test 未缓存。测试只覆盖纯 JVM 的 Protocol.kt 解析器（无 JNA/无 Windows）。
    testImplementation("junit:junit:4.13.2")
}

tasks.test {
    // Gradle 9 默认测试框架未定，显式选 JUnit 4（避免运行期告警/静默零执行）。
    useJUnit()
}

kotlin {
    compilerOptions {
        // Compose Desktop 要求 JVM target >= 11；不配置 toolchain（避免离线下载 JDK）。
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_11)
    }
}

// 共享源码：小欧的 Compose 绘制只有一份（shared/pet），与 android/app 同一份几何
// （candidate_io.rs 的线协议注释也是同样的「改协议须同步两处」约定）。
sourceSets["main"].kotlin.srcDir("../shared/pet")

java {
    // 与 Kotlin 的 JVM_11 对齐（本机 JDK 18 运行 Gradle，java 默认 target 18 会冲突）。
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}

compose.desktop {
    application {
        mainClass = "xyz.erik.opi.candidate.MainKt"

        nativeDistributions {
            // 构建验证用 Deb（Linux 主机，dpkg-deb 本机可用）；CMP 1.11 已移除
            // Zip 格式（枚举仅 AppImage/Deb/Rpm/Dmg/Pkg/Exe/Msi）。
            // Windows .msi 为验收阶段（Windows 主机上构建）。
            targetFormats(org.jetbrains.compose.desktop.application.dsl.TargetFormat.Deb)
            packageName = "opi-candidates"
            // 与发布 tag 对齐（同 android/app/build.gradle.kts 的 versionName）。
            packageVersion = "1.3.6"
            description = "OPI 拼音输入法候选窗（TSF 插件经 named pipe 通信）"
            vendor = "OPI"

            // 应用图标 = 项目宠物「小欧」，与 Android 启动图标、候选窗里摊手的那个是同一张图
            // （几何真源 docs/opi-pet.svg）。不挂的话装出来是 CMP 的 Kotlin 默认图标。
            //
            // 必须按平台分两份：CMP 把 iconFile **原样**作为 `--icon` 传给 jpackage，不做格式
            // 转换（反编译 AbstractJPackageTask 确认；它自带的默认图标也是每平台一份：jar 里
            // default-compose-desktop-icon-{linux.png,windows.ico,mac.icns}），而 jpackage 在
            // Linux 只认 .png、Windows 只认 .ico。用 `linux{}`/`windows{}` 分挂 = 构建主机是哪台
            // 就取哪份，与 targetFormats 的现实一致（Linux 主机打 Deb、Windows 主机打 Msi）。
            //
            // 两份都是渲染产物，**别手改**（改了下次重生成就无声回退）。重生成（在 desktop/ 下执行）：
            //   rsvg-convert -w 1000 -h 1042 ../docs/opi-pet.svg -o /tmp/pet.png
            //   magick /tmp/pet.png -background none -gravity center -extent 1042x1042 /tmp/m.png
            //   magick /tmp/m.png -strip -resize 256x256 icons/opi-pet.png
            //   magick /tmp/m.png -strip -define icon:auto-resize=256,128,64,48,32,16 icons/opi-pet.ico
            // SVG 的 viewBox 是 240x250（非正方形）——前两条只做「按比例渲染 + 居中留白到方形」，
            // 几何一个数没动；小欧还是那张图。
            // `-strip` 不是装饰：不加的话 imagemagick 会把本次生成的 date:create/date:modify 写进
            // PNG，同一张图每次重生成都得到不同字节（实测：像素签名相同、字节不同）—— 于是「没
            // 改过小欧」也会在 git 里显示成改动。加上才可复现（连跑两次 md5 相同）。
            // ⚠️ `appCategory` 是 **LinuxPlatformSettings** 上的属性（反编译该 class 确认），
            // 只能写在 `linux {}` **里面** —— 写到外层是 `Unresolved reference 'appCategory'`。
            //
            // ⚠️⚠️ **但实测它在 CMP 1.11.1 的 Deb 目标上不生效**，写在这里是「备着」不是「管用」：
            //   单变量实验 —— 把它设成 `"ZZZTEST"` 重建 .deb，`.desktop` 里**仍然是**
            //   `Categories=未知`（判据：`dpkg-deb --fsys-tarfile <deb> | tar -xO --wildcards
            //   '*.desktop' | grep Categories`；`.desktop` 的时间戳确认是本次生成的，非缓存）。
            //   `AbstractJPackageTask` 确实有 `linuxAppCategory` 属性、也确实传给 jpackage，
            //   但这条链接在 1.11.1 上不通。**别把它当成已修好的东西。**
            //
            // 「未知」多半是 jpackage 把它的默认值 `Unknown` 按本机 locale 翻译了 ——
            // 也就是说这个值**在任何 locale 下都是无效分类**（Desktop Entry 只认固定的英文
            // 集合）。影响面很小：候选窗是伴侣进程，用户不从菜单启动它，最坏是被归进
            // 「未分类」。真要修得走 jpackage 的 `--resource-dir` 换 `.desktop` 模板，
            // 那是另一件事，本轮没做。
            // `Utility` 是语义上正确的值 —— 留着它，CMP 哪天把链接接通就自动生效。
            linux {
                iconFile.set(layout.projectDirectory.file("icons/opi-pet.png").asFile)
                appCategory = "Utility"
            }
            windows { iconFile.set(layout.projectDirectory.file("icons/opi-pet.ico").asFile) }
        }
    }
}
