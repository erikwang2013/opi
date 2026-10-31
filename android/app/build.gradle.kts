// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "xyz.erik.opi"
    compileSdk = 34
    ndkVersion = "27.0.12077973"

    // 共享源码：小欧的 Compose 绘制只有一份（shared/pet），同时供本模块与
    // desktop/ 的 Windows 候选窗使用 —— 两处重复一份几何必然漂移。
    sourceSets.getByName("main").kotlin.srcDir("../../shared/pet")
    // 键盘动态背景（2026-10-30）：与小欧同一条「一份 Compose 绘制、两端共用」。
    sourceSets.getByName("main").kotlin.srcDir("../../shared/scene")

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
        applicationId = "xyz.erik.opi"
        minSdk = 21
        targetSdk = 34
        // 与发布 tag 对齐（此前 9 个 tag 期间 versionCode 一直是 1、versionName 一直是
        // "1.0.0"：APK 既无法被识别，也无法覆盖安装升级）。
        versionCode = 105
        versionName = "1.28.2"
        ndk {
            // 与 rust_builder cargokit targets 对齐（plugin.gradle 固定三 ABI）。
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    // 「关于」一节要读 BuildConfig.VERSION_NAME —— AGP 8.0 起 buildConfig 默认**不生成**，
    // 不显式打开就是编译期找不到符号。
    buildFeatures {
        buildConfig = true
    }

    // 分发签名 = 仓库内自带的社区签名密钥库（公开的 sideload 密钥，不是商店密钥；
    // 密码与警告见 android/keystore/README.md）。固定它，跨机器/跨 CI 的 APK
    // 才能互相覆盖安装 —— debug 签名来自每台机器各自的 ~/.android/debug.keystore，
    // 每次 CI 重建都换一张证书。
    signingConfigs {
        create("release") {
            storeFile = rootProject.file("keystore/opi-community.jks")
            storePassword = "opi-community"
            keyAlias = "opi-community"
            keyPassword = "opi-community"
            require(storeFile?.isFile == true) {
                "缺少社区签名密钥库：$storeFile（见 android/keystore/README.md）"
            }
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.getByName("release")
        }
    }
}

// ---------- 小欧表情贴纸 / GIF / 表（2026-10-18，2026-10-19 扩）----------
//
// 「自有」表情页点形态插入的图片（不是 Unicode 笑脸）。真源 =
// `shared/pet/stickers/*.png` 与 `shared/pet/stickers-gif/*.gif`
// （`scripts/gen_pet_stickers.py` 从 Compose 绘制直出透明底），构建期原样拷进
// assets/pet、assets/pet-gif —— 盘上没有第二份入库副本（同 desktop）。
// `shared/pet/emoji-font/` 只剩 map.tsv/shortcodes.tsv（标签与稳定码位 ID），
// 无字体文件；Android 侧见 PetEmojiTables.kt。
// 运行期 asset 路径 = `pet/` / `pet-gif/` / `pet-font/` + 文件名。
// 源缺**响亮地死**：少装贴纸只会表现成「点形态没反应」，没人会去查构建配置。
val copyPetStickers by tasks.registering(Copy::class) {
    val pngSrc = layout.projectDirectory.dir("../../shared/pet/stickers")
    val gifSrc = layout.projectDirectory.dir("../../shared/pet/stickers-gif")
    val tableSrc = layout.projectDirectory.dir("../../shared/pet/emoji-font")
    doFirst {
        check(pngSrc.asFile.isDirectory && gifSrc.asFile.isDirectory && tableSrc.asFile.isDirectory) {
            "找不到小欧表情资产：\n  ${pngSrc.asFile.path}\n  ${gifSrc.asFile.path}\n  " +
                "${tableSrc.asFile.path}\n生成一份：python3 scripts/gen_pet_stickers.py " +
                "&& python3 scripts/gen_pet_emoji_table.py"
        }
    }
    from(pngSrc) { into("pet") }
    from(gifSrc) { include("*.gif"); into("pet-gif") }
    from(tableSrc) {
        include("map.tsv", "shortcodes.tsv")
        into("pet-font")
    }
    into(layout.buildDirectory.dir("generated/opiAssets"))
}
// assets 根指向 `generated/opiAssets`（其下是 pet/ 等），运行期路径才是 pet/<file>.png。
// ⚠️ 必须 `.get().asFile`：AGP 9 的 SourceSet API 拒绝 Provider（报「cannot add Provider
// instances」），任务依赖由下一行显式 dependsOn 承担（不靠 Provider 自动推断）。
android.sourceSets.getByName("main").assets.srcDir(
    layout.buildDirectory.dir("generated/opiAssets").get().asFile,
)
// srcDir 只是声明目录，不带任务依赖；资产合并前必须先跑完拷贝。
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("Assets") }
    .configureEach { dependsOn(copyPetStickers) }
// 所有 lint 变体都读 assets（AGP 9 实测 assembleRelease 依次红在
// generateReleaseLintVitalReportModel、lintVitalAnalyzeRelease；逐个点名只会在下一个
// 变体再红，所以按名字含 "Lint" 一次覆盖）。
// ⚠️ 任务名是 `lintVitalAnalyzeRelease`（小写 l），匹配必须忽略大小写；按 "Lint"
// 精确匹配会漏掉它，下一次 `assembleRelease` 仍红。
tasks.matching { it.name.contains("Lint", ignoreCase = true) }
    .configureEach { dependsOn(copyPetStickers) }

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
