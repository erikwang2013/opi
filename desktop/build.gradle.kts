// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// C3：候选窗（Compose Desktop）构建配置。
// 仓库镜像在 settings.gradle.kts（与 android/ 同约定，仅 aliyun，无 google()）。
// ⚠️ 必须 import：在 Kotlin DSL 里 `java.security.MessageDigest` 的 `java` 会被解析成
// Gradle 的 `java` 扩展（JavaPluginExtension），不是包名 —— 实测报
// `Unresolved reference 'security'`（脚本编译期就红，不是运行期）。
// （2026-10-09 晚随桌面入口注入一起恢复 —— 用法见文件尾的任务。）
import java.security.MessageDigest

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

// ---------- 版本号单一来源（2026-10-09）----------
//
// jpackage 的包版本与设置窗标题栏都从这里取。**此前设置窗里写死过一份**
// （"1.4.0"），到 1.6.0 就成了摆在用户眼前的假话 —— 两处写死是漂移的充分条件，
// 生成一个常量是把它压成一处的最短路径（同 android 侧 versionName 的单一来源约定）。
val opiAppVersion = "1.25.1"

val generateOpiVersionSource by tasks.registering {
    val outDir = layout.buildDirectory.dir("generated/sources/opiVersion/kotlin")
    outputs.dir(outDir)
    val v = opiAppVersion
    doLast {
        val f = outDir.get().file("xyz/erik/opi/candidate/Version.kt").asFile
        f.parentFile.mkdirs()
        f.writeText(
            """
            // SPDX-FileCopyrightText: 2026 erik.xyz
            // SPDX-License-Identifier: MIT
            //
            // 生成文件，**勿手改** —— 版本号来源是 desktop/build.gradle.kts 的 opiAppVersion。

            package xyz.erik.opi.candidate

            /** 设置窗标题栏显示的版本；与 jpackage 的包版本同源。 */
            const val OPI_VERSION: String = "$v"
            """.trimIndent() + "\n",
        )
    }
}
// srcDir(taskProvider) = 「用这个任务的输出目录、并自动接上依赖」（Gradle 的既有语义）。
kotlin.sourceSets["main"].kotlin.srcDir(generateOpiVersionSource)

// ---------- 表情数据（2026-10-12）----------
//
// 桌面符号页的「表情」页要**整表** emoji（常用子集，桌面侧不再筛）—— 与引擎
// `builtin_emoji_texts()` 同一份表。
// 真源 = `data/raw/emoji.tsv`（`scripts/gen_emoji_kaomoji.py` 生成；引擎侧 `include_str!`
// 编进 .so，桌面是 JVM 进程读不到那一份）⇒ **构建期原样拷进资源**。
// ⚠️ 这不是「第二份入库副本」：复制发生在每次构建，盘上没有可漂的第二份（对照
// `crates/fcitx5-opi/data/symbol_categories.tsv` 那种真·双输出 —— 那个要 CI 逐字节比，
// 本文件不需要、也不该有那种比对）。运行期按 classpath 资源读，两端同一条路。
// 源文件不在 = **响亮地死**（与 CMakeLists 对 luna/符号类目源的 FATAL 同姿态）：
// 少装的表情页只会表现为「打开是空的」，没有人会去查构建配置。
val copyEmojiData by tasks.registering(Copy::class) {
    val src = file("../data/raw/emoji.tsv")
    doFirst {
        check(src.isFile) {
            "找不到表情数据源: ${src.path}\n" +
                "生成一份：python3 scripts/gen_emoji_kaomoji.py（写入 data/raw/emoji.tsv）"
        }
    }
    from(src)
    into(layout.buildDirectory.dir("generated/opiResources"))
}
sourceSets["main"].resources.srcDir(copyEmojiData)

// ---------- 颜文字数据（2026-10-18）----------
//
// 桌面表情页的「颜文字」子页与 Android 同一份数据：真源 = `data/raw/kaomoji.tsv`
// （`scripts/gen_emoji_kaomoji.py` 生成；Android 另有一份 assets 副本）。同 copyEmojiData
// 的纪律：**构建期原样拷进资源**、盘上没有可漂的第二份；源文件不在 = 响亮地死。
// ⚠️ 与 copyEmojiData 写同一个输出目录 ⇒ **不能再注册一次 srcDir**（processResources
// 会把目录里每个文件扫两遍，报 duplicate entry；2026-10-16 copyKangxiDict 实测过）。
// 接线用 dependsOn，文件进同一个目录、目录只扫一次。
val copyKaomojiData by tasks.registering(Copy::class) {
    val src = file("../data/raw/kaomoji.tsv")
    doFirst {
        check(src.isFile) {
            "找不到颜文字数据源: ${src.path}\n" +
                "生成一份：python3 scripts/gen_emoji_kaomoji.py（写入 data/raw/kaomoji.tsv）"
        }
    }
    from(src)
    into(layout.buildDirectory.dir("generated/opiResources"))
}
tasks.named("processResources") { dependsOn(copyKaomojiData) }

// ---------- Windows TSF DLL：随安装器一起装（2026-10-18 用户裁决）----------
//
// 用户原话「DLL 不要单独发布，用户不会用」：DLL 是真正的 Windows 输入法 COM 组件，
// 单独挂一个 .dll 资产等于让用户自己 regsvr32 —— 他们不会。这里把它挂进 jpackage 的
// `--input` 文件集合，随应用镜像进入 MSI / EXE 安装包；MSI 的注册动作由
// `scripts/postprocess-windows-msi.sh` 注入，EXE 由 `scripts/windows/opi.iss` 的
// `[Code]` 在文件落盘后执行。
//
// Windows CI 在 `packageMsi` / `createDistributable` 前下载 TSF DLL artifact 并把路径放进 OPI_TSF_DLL；
// Linux 打 Deb 时该变量不存在 ⇒ files 集合不变，不会把 DLL/脚本/词库带进 Deb。
val tsfDll = providers.environmentVariable("OPI_TSF_DLL")
// jpackage 的 `--input` 是 CMP 自己暂存的 libs 目录（会清空重填），所以不能往里
// 手写文件；把 DLL 挂到 JPackage 任务的 `files` 集合，CMP 会与 jar 一起暂存并
// 打进应用镜像。Windows CI 在 packageMsi 前设 OPI_TSF_DLL；Linux 构建不设。
tasks.withType<org.jetbrains.compose.desktop.application.tasks.AbstractJPackageTask>()
    .configureEach {
        if (tsfDll.isPresent) {
            val src = file(tsfDll.get())
            check(src.isFile) { "OPI_TSF_DLL 指向的文件不存在：${src.path}" }
            // 完成页复选框拉起这个脚本：只启动候选窗，**不碰 Explorer**（2026-10-20 白屏后）。
            // 只在这个分支挂入 ⇒ Linux Deb 不会带 Windows 脚本。
            // 2026-10-20：项目不再随包发布 PUA 字体（见 shared/pet/emoji-font/README.md）。
            //
            // 词库（2026-10-20）：jpackage 把 `--input` 的普通文件放进 app image 的 `app\`，
            // 即 `app\luna.opid` 与 `app\tsf_opi.dll` 同目录 —— 正是 `dict_path` 的第二候选。
            // 真源 = 入库的 Android 分发副本（见 `crates/tsf-opi/src/dict_path.rs` 模块头）。
            // 许可文本同批进包：`luna.opid` 由 LGPL/Apache-2.0/GPL-3.0-only 数据编译，
            // 分发义务与 `scripts/nfpm.yaml` 那段相同（少了它们包在授权上不完整）。
            val luna = file("../android/app/src/main/assets/luna.opid")
            check(luna.isFile) { "找不到词库分发副本：${luna.path}" }
            // 符号类目数据（2026-10-21）：状态栏「符号大全」按钮在 Windows 上原本只能打开
            // 一个「没有类目数据」的空页 —— 数据没随包。真源 = Android 分发副本（与 luna
            // 同一条纪律）；jpackage 把它放进 `app\`，`symbolDataCandidates` 加了该顺位。
            val symbols = file("../android/app/src/main/assets/symbol_categories.tsv")
            check(symbols.isFile) { "找不到符号类目分发副本：${symbols.path}" }
            val licenses = listOf(
                file("../LICENSE"),
                file("../data/raw/LICENSES.md"),
                file("../data/raw/GPL-3.0.txt"),
            )
            licenses.forEach { check(it.isFile) { "找不到许可文本：${it.path}" } }
            files.from(
                src,
                file("windows/launch-and-restart.cmd"),
                luna,
                symbols,
                *licenses.toTypedArray(),
            )
        }
    }

// 康熙字典词库（2026-10-16，「全发」的桌面面）：词库页首启种子把它放进词库目录一次
//（见 Dicts.kt 的 seedBundledDict）。真源 = `data/raw/kangxi.tsv`
//（`scripts/gen_kangxi_dict.py` 生成；Android/包/desktop 三处消费同一份产物）。
// 与 copyEmojiData 同一条纪律：构建期原样拷，盘上没有可漂的第二份；源缺**响亮地死**。
val copyKangxiDict by tasks.registering(Copy::class) {
    val src = file("../data/raw/kangxi.tsv")
    doFirst {
        check(src.isFile) {
            "找不到康熙字典词库源: ${src.path}\n" +
                "生成一份：python3 scripts/gen_kangxi_dict.py"
        }
    }
    from(src)
    into(layout.buildDirectory.dir("generated/opiResources"))
}
// ⚠️ 同一个资源目录**不能注册两次** srcDir（上面 copyEmojiData 已注册过它）——
// 2026-10-16 实测：再 `srcDir(copyKangxiDict)` 会让 processResources 报
// 「Entry kangxi.tsv is a duplicate」（目录里的每个文件都会被扫两遍）。
// 接线改为 dependsOn：文件进同一个处理过的目录、目录只扫一次。
tasks.named("processResources") { dependsOn(copyKangxiDict) }

// ---------- 小欧表情贴纸（2026-10-18）----------
//
// 「自有」表情页点形态要插入**图片**（不是 Unicode 笑脸）。真源 =
// `shared/pet/stickers/*.png`，由 `scripts/gen_pet_stickers.py` 从 `shared/pet` 的
// Compose 绘制直出（透明底）；Android 侧同一份源拷进 assets（见 app/build.gradle.kts）。
// 同 copyEmojiData 的纪律：构建期原样拷进资源、盘上没有可漂的第二份；源缺**响亮地死**。
// ⚠️ 与 copyEmojiData 共用输出目录 ⇒ **只 dependsOn、不再注册 srcDir**（重复注册同一个
// 目录会让 processResources 把每个文件扫两遍、报 duplicate entry，2026-10-16 实测过）。
// 运行期按 classpath 读 `/pet/<file>.png`（文件名的唯一真源 = PetStickerAssets）。
val copyPetStickers by tasks.registering(Copy::class) {
    val pngSrc = file("../shared/pet/stickers")
    val gifSrc = file("../shared/pet/stickers-gif")
    val fontSrc = file("../shared/pet/emoji-font")
    doFirst {
        check(pngSrc.isDirectory && gifSrc.isDirectory && fontSrc.isDirectory) {
            "找不到小欧表情资产：\n  ${pngSrc.path}\n  ${gifSrc.path}\n  ${fontSrc.path}\n" +
                "生成一份：python3 scripts/gen_pet_stickers.py && python3 scripts/gen_pet_emoji_table.py"
        }
    }
    from(pngSrc) { into("pet") }
    from(gifSrc) { include("*.gif"); into("pet-gif") }
    from(fontSrc) {
        include("map.tsv", "shortcodes.tsv")
        into("pet-font")
    }
    into(layout.buildDirectory.dir("generated/opiResources"))
}
tasks.named("processResources") { dependsOn(copyPetStickers) }

// 共享源码：小欧的 Compose 绘制只有一份（shared/pet），与 android/app 同一份几何
// （candidate_io.rs 的线协议注释也是同样的「改协议须同步两处」约定）。
sourceSets["main"].kotlin.srcDir("../shared/pet")

java {
    // 与 Kotlin 的 JVM_11 对齐（本机 JDK 18 运行 Gradle，java 默认 target 18 会冲突）。
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}

// ---------- 桌面入口：注入已**恢复**（2026-10-09 晚，用户裁决） ----------
//
// 时间线（两次裁决都留在这里，别删任何一段 —— 它是「两层都要能设置」的由来）：
//   · 2026-10-09 白天：「设置功能不能在外层设置，必须到配置里才有设置入口」
//     ⇒ 撤掉了注入（下面的任务曾被整块删除）。
//   · 2026-10-09 晚：「设置功能要在外层可以设置」⇒ 恢复注入。
//   合起来的要求 = **两层都要能设置**：fcitx5 的配置面（控制中心 → 输入法 →
//   高级设置 → 附加组件 → OPI → 齿轮；选项声明在 crates/fcitx5-opi/cpp/opi_config.h，
//   那条链与本次恢复无关、照旧）+ 这个系统菜单入口。
//   ⚠️ 「配置」按钮拉不起外部程序这一点没变（fcitx5 的 `Instance::configureAddon`
//   是 no-op —— 上游源码；dbus 侧只转调 `configure()` = 启动配置工具本身）：
//   这个入口只走**菜单**，两层的入口各自独立。
// 应用本体两端并用：Windows = 候选窗（TSF 命名管道两端在那边），Linux = 设置窗
// （`--settings`；无参数启动也是它）。

compose.desktop {
    application {
        mainClass = "xyz.erik.opi.candidate.MainKt"

        nativeDistributions {
            // 构建验证用 Deb（Linux 主机，dpkg-deb 本机可用）；CMP 1.11 已移除
            // Zip 格式（枚举仅 AppImage/Deb/Rpm/Dmg/Pkg/Exe/Msi）。
            //
            // Msi（2026-10-09 加）：Windows 侧由 .github/workflows/windows.yml 在
            // windows runner 上跑 `gradlew.bat packageMsi` 打出 `.msi`。
            // ⚠️ 加它的**理由**是机制性的，不是偏好：CMP 1.11.1 只为 `targetFormats` 里
            // **列出的**格式注册 `package<格式>` 任务（反编译 configurePackagingTasks：
            // 逐项遍历 getTargetFormats() 再 register）。实测：只列 Deb 时 `packageMsi`
            // **不存在** —— `Task 'packageMsi' not found ... candidates: 'package',
            // 'packageDeb'`，不是「任务在、只是跨平台打不出来」。
            // Linux 侧不受影响（实测过 configuration 期）：Deb 照旧；Msi 的任务在 Linux
            // 上不会被 `package`/`packageDistributionForCurrentOS` 选中（按当前 OS 过滤），
            // 且 CMP 的 WiX 接线（configureWix）整个只在 Windows 主机上跑。
            targetFormats(
                org.jetbrains.compose.desktop.application.dsl.TargetFormat.Deb,
                org.jetbrains.compose.desktop.application.dsl.TargetFormat.Msi,
            )
            // 2026-10-20 用户裁决：所有端的包/安装器/启动器一律叫 opi（不再叫
            // opi-candidates）—— Windows 默认装到 `C:\Program Files\opi`，Linux 的
            // deb/rpm 装到 `/opt/opi/bin/opi`（packages.yml / 打包脚本 / 文档同一落点）。
            // 不按构建主机分叉：jpackage 不能交叉打包（见 windows.yml 头注释），
            // 一台机器只会打自己那端的产物，同一个名字两边都成立。
            packageName = "opi"
            // 与发布 tag 对齐（同 android/app/build.gradle.kts 的 versionName）；
            // 值来自文件上方的 opiAppVersion 单一来源。
            packageVersion = opiAppVersion
            // ⚠️ description / vendor / packageName 这几个值**必须是纯 ASCII** ——
            // 它们在 Windows 上被 jpackage 写进 **MSI 数据库**，而数据库代码页由 WiX 的
            // 本地化文件决定：jpackage 内置的 en-us 那份写死 Codepage 1252（Windows
            // Installer 不支持 65001/UTF-8），于是任何中文/全角字符都让 light.exe 以
            // **LGHT0311** 退出。2026-10-09 在 windows runner 上实测到了：描述原文案
            // 「小欧输入法 候选窗（TSF 插件经 named pipe 通信）」→ 退出码 311，而 Gradle
            // 只转述「Exit code: 1」，真报错在
            // desktop/build/compose/logs/packageMsi/*.txt 里（windows.yml 有一条守卫
            // 会在这些值被改回非 ASCII 时直接红，省掉「light.exe 311 是什么」）。
            // 想给 Windows 中文文案：自带 MsiInstallerStrings_*.wxl 走 --resource-dir，
            // 但那份 Culture 不能是 en-us（否则又落回 1252）⇒ 得连 jpackage 选的 locale
            // 一起换 —— runner 上做不到（jpackage 按系统 locale 选，en-US 上 LANG 无效）。
            // 代价：.deb 的 Description 跟着一起变英文 —— CMP 的 DSL 只有**一处**
            // description，没有按平台分设的接口（javap 核过：AbstractDistributions 全局
            // 一处；WindowsPlatformSettings 上没有）。
            description = "opi IME candidate window (TSF plugin over a named pipe)"
            vendor = "opi"

            // 应用图标 = 项目宠物「小欧」，与 Android 启动图标、候选窗里摊手的那个是同一张图
            // （几何真源 docs/opi-pet.svg）。不挂的话装出来是 CMP 的 Kotlin 默认图标。
            //
            // 必须按平台分两份：CMP 把 iconFile **原样**作为 `--icon` 传给 jpackage，不做格式
            // 转换（反编译 AbstractJPackageTask 确认；它自带的默认图标也是每平台一份：jar 里
            // default-compose-desktop-icon-{linux.png,windows.ico,mac.icns}），而 jpackage 在
            // Linux 只认 .png、Windows 只认 .ico。用 `linux{}`/`windows{}` 分挂 = 构建主机是哪台
            // 就取哪份，与 targetFormats 的现实一致（Linux 主机打 Deb、Windows 主机打 Msi）。
            //
            // 两份都是生成物，**别手改**（改了下次重生成就无声回退）。源头是
            // `scripts/gen_pet_icons.py`（纸白底 + 6% 留白），重出：
            //   python3 scripts/gen_pet_icons.py           # 全部图标产物
            //   python3 scripts/gen_pet_icons.py --check   # 只对账，不一致退出 1
            // 旧注释里的 magick 配方已退役：它不铺纸白底，产物与生成器不一致。
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

// ---------- 「属性设置」的桌面入口（打包后注入）----------
//
// 为什么不交给 jpackage：`--linux-shortcut` 生成的 .desktop 用 `--name`
// （opi）当 Name —— 菜单里显示成那样既不像输入法/设置程序、也叫不准；
// 而且它**没法给入口带参数**。jpackage 的 `--resource-dir` 换模板是更正统的路，
// 但它要猜 jpackage 对模板文件的命名约定；这里选**打包后注入**：拆包 → 写一个
// .desktop → 重算 md5sums → 打回去。每一步都能用 `dpkg-deb -c` 核对。
//
// ⚠️ 注入发生在 **desktop 自己的 .deb** 上，之后 packages.yml 的合并步骤
// （scripts/build-packages.sh 的 OPI_DESKTOP_DEB）把这个 .deb 的数据文件解进
// 主包暂存树 —— 所以主包里的 /usr/share/applications/opi-settings.desktop
// 就是这么来的。
//
// 同一个任务还注入状态栏的自启项 /etc/xdg/autostart/opi-statusbar.desktop
// （`--statusbar`；2026-10-10 lead 裁决走系统级目录）。
//
// 调用（packages.yml 的 desktop-deb job 用的就是这条）：
//   ./gradlew packageDeb injectSettingsDesktopEntry --no-daemon
tasks.register("injectSettingsDesktopEntry") {
    dependsOn("packageDeb")
    doLast {
        val debDir = layout.buildDirectory.dir("compose/binaries/main/deb").get().asFile
        val deb = debDir.listFiles { f: File -> f.name.endsWith(".deb") }?.maxByOrNull { it.name }
            ?: error("找不到 jpackage 出的 .deb —— 先跑 :desktop:packageDeb")
        val work = layout.buildDirectory.dir("tmp/settings-entry").get().asFile
        work.deleteRecursively()
        work.mkdirs()

        fun sh(vararg cmd: String) {
            val proc = ProcessBuilder(*cmd).redirectErrorStream(true).start()
            val out = proc.inputStream.readBytes().decodeToString()
            check(proc.waitFor() == 0) { "${cmd.joinToString(" ")} 失败：\n$out" }
        }

        val root = File(work, "root")
        sh("dpkg-deb", "-R", deb.absolutePath, root.absolutePath)

        // 从**包里**取真正的启动器路径（不硬编码 —— jpackage 换布局时这里跟着走）
        val bin = File(root, "opt").listFiles()?.firstOrNull()
            ?.resolve("bin")?.listFiles()?.firstOrNull()
            ?: error("包里没有 /opt/*/bin/* 启动器")
        val installedBin = "/opt/" + bin.toRelativeString(File(root, "opt"))
        // Icon **不再**指 jpackage 那份 /opt/*/lib/*.png（栅格图：同一只小欧，却是
        // 构建产物、没进 git）。改指 hicolor 里的浅底矢量口径 docs/opi-pet-app.svg —— 由
        // crates/fcitx5-opi/cpp/CMakeLists.txt 把 docs/opi-pet-app.svg
        // 装到 /usr/share/icons/hicolor/scalable/apps/opi-pet-app.svg，packages.yml 的
        // 合并步把它并进主包 ⇒ 装完必在（2026-10-10 lead 核过；发布物是合并后的
        // 主包，desktop 的 .deb 单独装时这份不在）。用户裁决同日：菜单/桌面入口的
        // 图标也要是 logo。窗口图标（Painter）在那边是另一份，见 PetIcon.kt。
        val installedIcon = "/usr/share/icons/hicolor/scalable/apps/opi-pet-app.svg"

        val apps = File(root, "usr/share/applications").apply { mkdirs() }
        // 若 jpackage 自己那份也在（同一 --name，如 opi.desktop），删掉：
        // 两份入口 = 菜单里两条，且那条不带 --settings。
        apps.listFiles { f: File -> f.name.endsWith(".desktop") }
            ?.filter { it.name != "opi-settings.desktop" }?.forEach { it.delete() }
        // Name = 「小欧输入法」（2026-10-09 用户裁决：所有端安装后显示名统一为它）。
        // 菜单入口与 Deepin 上其它输入法的菜单条目同款：点开就是设置窗。
        File(apps, "opi-settings.desktop").writeText(
            """
            [Desktop Entry]
            Type=Application
            Name=小欧输入法
            Comment=小欧输入法 —— 学习 / 中文标点 / 模糊音 / 简拼 / Shift 切换中英文 / 皮肤
            Comment[en]=opi IME — learning, punctuation, fuzzy pinyin, jianpin, shift toggle, skins
            Exec=$installedBin --settings
            Icon=$installedIcon
            Terminal=false
            Categories=Utility;Settings;
            Keywords=ime;input;fcitx5;opi;settings;
            """.trimIndent() + "\n",
        )

        // 状态栏的自启项（2026-10-10 lead 裁决：走**系统级** /etc/xdg/autostart/，
        // 随本包一起装 —— 不用 ~/.config/autostart 是因为那要每用户写一次，
        // 而安装包只有 root 那一次机会）。
        // · Exec 带 --statusbar（`--settings` 那份是菜单入口，两者别混）；
        // · 条上的 master 开关只隐藏悬浮条，进程照旧活着轮询（见 StatusBar.kt）；
        // · Windows 那侧没有这个文件（.msi 里没有 XDG 这一套），也就没有触发器。
        val autostart = File(root, "etc/xdg/autostart").apply { mkdirs() }
        File(autostart, "opi-statusbar.desktop").writeText(
            """
            [Desktop Entry]
            Type=Application
            Name=小欧输入法 状态栏
            Comment=小欧输入法悬浮状态栏（软键盘 / 符号 / 简繁 / 标点 / 中英 / 全半 / 设置）
            Exec=$installedBin --statusbar
            Icon=$installedIcon
            Terminal=false
            NoDisplay=true
            X-GNOME-Autostart-enabled=true
            """.trimIndent() + "\n",
        )

        // md5sums 必须重算：改了内容不更新它，`dpkg --verify` 会报一整包文件被改
        // （`dpkg -i` 本身不看它，所以这是个**只在事后暴露**的坑）。
        val md5 = File(root, "DEBIAN/md5sums")
        if (md5.parentFile.isDirectory) {
            val lines = root.walkTopDown()
                .filter { it.isFile && !it.path.startsWith(File(root, "DEBIAN").path) }
                .sortedBy { it.path }
                .map { f ->
                    val rel = f.toRelativeString(root)
                    val sum = MessageDigest.getInstance("MD5")
                        .digest(f.readBytes()).joinToString("") { "%02x".format(it) }
                    "$sum  $rel"
                }
                .toList()
            md5.writeText(lines.joinToString("\n", postfix = "\n"))
        }

        sh("dpkg-deb", "--build", "--root-owner-group", root.absolutePath, deb.absolutePath)
        println("已注入桌面入口：$installedBin --settings  ->  ${deb.name}")
        println("已注入状态栏自启：$installedBin --statusbar  ->  /etc/xdg/autostart/opi-statusbar.desktop")
        println("核对：dpkg-deb -c ${deb.absolutePath} | grep -E 'applications|autostart|bin/'")
    }
}

