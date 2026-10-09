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
val opiAppVersion = "1.7.4"

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
            packageName = "opi-candidates"
            // 与发布 tag 对齐（同 android/app/build.gradle.kts 的 versionName）；
            // 值来自文件上方的 opiAppVersion 单一来源。
            packageVersion = opiAppVersion
            description = "opi输入法 候选窗（TSF 插件经 named pipe 通信）"
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

// ---------- 「属性设置」的桌面入口（打包后注入）----------
//
// 为什么不交给 jpackage：`--linux-shortcut` 生成的 .desktop 用 `--name`
// （opi-candidates）当 Name —— 菜单里显示成那样既不像输入法/设置程序、也叫不准；
// 而且它**没法给入口带参数**。jpackage 的 `--resource-dir` 换模板是更正统的路，
// 但它要猜 jpackage 对模板文件的命名约定；这里选**打包后注入**：拆包 → 写一个
// .desktop → 重算 md5sums → 打回去。每一步都能用 `dpkg-deb -c` 核对。
//
// ⚠️ 注入发生在 **desktop 自己的 .deb** 上，之后 packages.yml 的合并步骤
// （scripts/build-packages.sh 的 OPI_DESKTOP_DEB）把这个 .deb 的数据文件解进
// 主包暂存树 —— 所以主包里的 /usr/share/applications/opi-settings.desktop
// 就是这么来的。
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

        // 从**包里**取真正的启动器与图标路径（不硬编码 —— jpackage 换布局时这里跟着走）
        val bin = File(root, "opt").listFiles()?.firstOrNull()
            ?.resolve("bin")?.listFiles()?.firstOrNull()
            ?: error("包里没有 /opt/*/bin/* 启动器")
        val icon = bin.parentFile.parentFile.resolve("lib").listFiles()
            ?.firstOrNull { it.name.endsWith(".png") }
            ?: error("包里没有 /opt/*/lib/*.png 图标")
        val installedBin = "/opt/" + bin.toRelativeString(File(root, "opt"))
        val installedIcon = "/opt/" + icon.toRelativeString(File(root, "opt"))

        val apps = File(root, "usr/share/applications").apply { mkdirs() }
        // 若 jpackage 自己那份也在（同一 --name，如 opi-candidates.desktop），删掉：
        // 两份入口 = 菜单里两条，且那条不带 --settings。
        apps.listFiles { f: File -> f.name.endsWith(".desktop") }
            ?.filter { it.name != "opi-settings.desktop" }?.forEach { it.delete() }
        // Name = 「opi输入法」（2026-10-09 用户裁决：所有端安装后显示名统一为它）。
        // 菜单入口与 Deepin 上搜狗的「搜狗输入法」条目同款：点开就是设置窗。
        File(apps, "opi-settings.desktop").writeText(
            """
            [Desktop Entry]
            Type=Application
            Name=opi输入法
            Comment=opi输入法 —— 学习 / 中文标点 / 模糊音 / 简拼 / Shift 切换中英文 / 皮肤
            Comment[en]=opi IME — learning, punctuation, fuzzy pinyin, jianpin, shift toggle, skins
            Exec=$installedBin --settings
            Icon=$installedIcon
            Terminal=false
            Categories=Utility;Settings;
            Keywords=ime;input;fcitx5;opi;settings;
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
        println("核对：dpkg-deb -c ${deb.absolutePath} | grep -E 'applications|bin/'")
    }
}

