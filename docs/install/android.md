<!-- SPDX-FileCopyrightText: 2026 erik.xyz -->
<!-- SPDX-License-Identifier: MIT -->

# Android 端（原生 IME）编译 / 打包 / 安装 / 使用

> ### 先读这一段
>
> 本机有 Android SDK + NDK，还接着一台 **x86_64 模拟器（API 36）** —— `adb` 与
> `./gradlew` 都能真跑，所以本文多数条目是**【实测】**（三个有端代码的平台里，
> Windows 那端本机完全没有条件，见 `docs/install/windows.md`）。
>
> **但有一条硬前提本机没满足：`./gradlew assembleDebug` 在本机是失败的** —— 缺 Dart SDK
> （cargokit 的构建工具是 Dart 程序），见 §2.4。这不是仓库坏了，是工具链不全。
> 照本文走之前，请先看 §2.4。
>
> 每条都带证据档位：
>
> | 标记 | 含义 |
> |---|---|
> | **【实测】** | 2026-09-28 在本机（Linux + x86_64 模拟器）真跑过，命令与输出在文内 |
> | **【推断】** | 从源码读出来的结论，给出「文件 + 符号」，没有运行过 |
> | **【未验证】** | 本机没有条件跑，或确实没跑 |
>
> 引用一律写「文件 + 符号」，不写行号（并发改动会让裸行号失准）。**版本号、条数、
> 字节数一律不当「现值」写**：要么只给读它的命令，要么明确标成「某天某条命令的输出」
> —— 这类数字会随数据扩容静默漂移，仓库里已经有几处正在漂（见 §6.3）。

---

## 1. 编译前提

### 1.1 需要装什么

| 项 | 值 / 命令 | 依据 |
|---|---|---|
| JDK | **字节码目标 17**（`sourceCompatibility` / `targetCompatibility` / `jvmTarget`），本机跑的是 JDK 18 | `android/app/build.gradle.kts` 的 `compileOptions` 与 `kotlin.compilerOptions`。**AGP 9 要求的 JDK 下界本文未实测**（本机只有一个 JDK） |
| Android SDK | `compileSdk = 34`、`targetSdk = 34`、`minSdk = 21` | `android/app/build.gradle.kts` 的 `android` 块与 `defaultConfig` |
| Android NDK | `27.0.12077973`（**两个构建根各写了一次，必须同值**） | `android/app/build.gradle.kts` 的 `ndkVersion`；`android/rust_builder/android/build.gradle` 的 `ndkVersion`。后者注释写明「本机仅此一个」 |
| Rust | **≥ `rust-version = "1.88"`**（根 `Cargo.toml` 的 `[workspace.package]`） | 卡住下界的不是 edition 2024（只要 1.85），是 let-chain（1.88 才稳定），理由写在同一处 `rust-version` 上方 |
| Rust 三个目标 | `aarch64-linux-android` · `armv7-linux-androideabi` · `x86_64-linux-android` | cargokit 固定编三个（`rust_builder/cargokit/gradle/plugin.gradle` 的 `platforms`）。`rustup target add` 这三条 |
| **Dart SDK**（或 `FLUTTER_ROOT`） | **本项目的隐藏前提**，见 §2.4 | `rust_builder/cargokit/run_build_tool.sh` 用 `dart pub get` + `dart compile kernel` 编译它自己的构建工具；`plugin.gradle` 的 `CargoKitBuildTask` 调这个脚本 |
| Gradle | 不用单独装：`gradlew` 用 wrapper，本机已缓存 `gradle-9.1.0-all`（`android/gradle/wrapper/gradle-wrapper.properties` 的 `distributionUrl`） | 【实测】`~/.gradle/wrapper/dists/` 里有 `gradle-9.1.0-all`；本机 `which gradle` 零命中 —— **只能用 `./gradlew`** |
| 网络 | maven 仓库指向阿里云镜像 | `android/settings.gradle.kts` 与 `rust_builder/android/build.gradle`：本机 `dl.google.com` 被 DNS 劫持到 ~2KB/s，故**绝不写 `google()`** |

### 1.2 本机实测环境（2026-09-28）

```
java -version           → JDK 18.0.2.1（/usr/lib/jvm/jdk-18.0.2.1）
ANDROID_HOME            → /home/component/Android/sdk
sdk/ndk/                → 27.0.12077973（仅此一个）
sdk/platforms/          → android-34 · android-36 · android-Baklava
sdk/build-tools/        → 34.0.0 · 36.0.0 · 36.1.0 · 37.0.0
rustc / cargo           → 1.97.1
rustup target installed → 含 aarch64-linux-android / armv7-linux-androideabi / x86_64-linux-android
dart / flutter          → 零命中；FLUTTER_ROOT 未设置        ← §2.4 的成因
adb devices             → emulator-5554  device（x86_64，ro.build.version.sdk=36）
```

### 1.3 三个 ABI 与它们的关系

三处名字不一样，**说的是同一件事**，加 ABI 要同时改两处硬编码：

| Rust 目标（`rustup target`） | cargokit 名（`rust_builder/cargokit/gradle/plugin.gradle` 的 `platforms`） | Android ABI（`app/build.gradle.kts` 的 `abiFilters`） | 谁在用 |
|---|---|---|---|
| `aarch64-linux-android` | `android-arm64` | `arm64-v8a` | 现代真机（绝大多数） |
| `armv7-linux-androideabi` | `android-arm` | `armeabi-v7a` | 老 32 位真机 |
| `x86_64-linux-android` | `android-x64` | `x86_64` | **本机这台模拟器**（【实测】`ro.product.cpu.abi=x86_64`） |

映射关系在 `rust_builder/cargokit/build_tool/lib/src/target.dart` 的 `Target.all` 里（`rust` / `flutter` / `android` 三个字段）。cargokit 名字 → ABI 的对应关系就在那张表，别自己猜。

- 【推断】`abiFilters` 与 `platforms` **是两处独立的硬编码**，谁都不读谁。改一处不改另一处，最可能的结果不是报错而是「某个 ABI 的 `.so` 没进 APK」—— 装机时才炸。
- 【推断】`minSdk = 21` 与 NDK 27 的关系写死在 `rust_builder/android/build.gradle` 的 `defaultConfig` 注释里：NDK 27 的最低支持就是 API 21（API 19 的 sysroot 不存在，链接 `crtbegin_so.o` 失败）。

### 1.4 SDK 路径：两份 `local.properties` 与 `ANDROID_HOME`

**`android/local.properties` 与 `android/rust_builder/local.properties` 是两份文件，都被 `.gitignore` 忽略**（【实测】`git check-ignore` 对两者都命中 `android/.gitignore` 的 `local.properties` 行），内容是 `sdk.dir=...`。

- `android/local.properties` 由 AGP 自己读（app 侧）。
- `android/rust_builder/local.properties` 由 **cargokit 自己读**：`plugin.gradle` 的 `CargoKitPlugin.resolveSdkDir` 取的是**它自己那个构建根**的 `local.properties`（`rust_builder` 是 `includeBuild` 的独立构建，有各自的 `settings.gradle`），读不到才回退环境变量 `ANDROID_HOME` → `ANDROID_SDK_ROOT` → 抛 `GradleException("Android SDK location not found ...")`。
- 【实测】本机两份都存在，且 `ANDROID_HOME` 也有值 —— **所以本机分不清是哪一条在起作用**。想稳，两条都写（或保证 `ANDROID_HOME` 有值）。

### 1.5 词库是构建输入，不是构建产物

`android/app/src/main/assets/` 下的 `luna.opid`（简体）与 `trad.opid`（繁体）**是入库的二进制文件**，由 `data/raw/*.tsv` 经 `opi-tools` 编译而来。**Android 的构建流程不会重新生成它们** —— 换了 `data/raw` 而不重新编译词库，APK 里装的还是旧词库。

---

## 2. 编译与打包

### 2.1 命令（**`gradlew` 在 `android/` 下，仓库根没有它**）

```bash
cd android                      # 必须。仓库根、desktop/ 的 gradlew 都不是这个工程

./gradlew testDebugUnitTest     # JVM 单测（引擎控制器 / IME 状态机 / 键盘路由 / 用户词存储 / 词库装载 / 宠物）
./gradlew assembleDebug         # 打 debug APK（cargokit 顺带编 opi-ffi 三 ABI .so）
./gradlew assembleRelease       # 打 release APK（未实测，见 §2.3）
./gradlew clean                 # 根 build.gradle.kts 里注册的 Delete 任务
```

【实测】`./gradlew testDebugUnitTest` → `BUILD SUCCESSFUL`，EXIT=0。**它不需要 Dart、也不需要 `.so`**：

- 任务图里**没有** cargokit 那条（`./gradlew testDebugUnitTest --dry-run` → EXIT=0，`grep -c cargokit` 为 **0**）。对照：`./gradlew assembleRelease --dry-run` 的任务图里**有** `:rust_builder:android:cargokitCargoBuildOpi_ffiRelease`（`grep -c cargokit` 为 1）。
- 原因是单测用的是假引擎与假 `FileOps`（`EngineLoader` 的文件操作与加载抽象就是为此抽出来的，见 `android/app/src/test/kotlin/io/opi/input/jni/EngineLoaderTest.kt`）。

> 老实说清这次跑到的程度：本机这一次 `testDebugUnitTest` 是 **`UP-TO-DATE`**（此前跑过、输入没变，Gradle 跳过了执行）—— 所以 EXIT=0 证明的是「任务图能跑通、不依赖 cargokit」，**不是「测试真的重跑了一遍且全绿」**。要后者加 `--rerun-tasks`。

### 2.2 产物在哪

| 产物 | 路径 |
|---|---|
| APK | `android/app/build/outputs/apk/debug/app-debug.apk` |
| APK 元数据（含 versionCode / versionName） | `android/app/build/outputs/apk/debug/output-metadata.json` |
| Rust `.so`（每 ABI 一份，先落这里再进 APK） | `android/rust_builder/android/build/jniLibs/debug/<abi>/libopi_ffi.so` |
| CI/排障用问题报告 | `android/build/reports/problems/problems-report.html` |

`android/` 下的 `build/`、`.gradle/`、`local.properties` 都在 `.gitignore` 里（`android/.gitignore`）——**别把这些产物提交上去**。

### 2.3 debug 与 release 的差别

`android/app/build.gradle.kts` 的 `buildTypes.release` 只有一行配置，注释原文：

> 本环境无 flutter_embedding_release 工件，只构建 debug；release 暂用 debug 签名保证可打包。

如实转述三点：

1. **release 目前用 debug 签名**（`signingConfig = signingConfigs.getByName("debug")`）。后果：release APK 与 debug APK 同签名，能互相覆盖安装 —— 但**不能上任何商店**，也不能换成正式签名后覆盖安装（换签名必须先卸载，`INSTALL_FAILED_UPDATE_INCOMPATIBLE`）。
2. 那条注释里的**理由措辞是 Flutter 时期的**（`android/` 下早已没有 flutter 目录，Flutter 方案随 M6a 原生重构作废，见 `docs/superpowers/plans/2026-08-12-opi-ime-android-m4.md` 头部的订正注记）。**「release 用 debug 签名」这个事实仍然有效 —— 读的是代码，不是那段理由。**
3. 【未验证】release 变体本机**没有真正构建过**（本机连 debug 都过不去，见 §2.4）。【实测】仅跑过 `./gradlew assembleRelease --dry-run`（EXIT=0，只出任务图，**不证明能编出来**）。

debug 签名的证书是哪一张（本机实测）：

```
apksigner verify --print-certs app-debug.apk
  → Signer #1 certificate DN: C=US, O=Android, CN=Android Debug
    SHA-256: d9cb1b0c5794b7ee75144ab7876aec8c424fcdcba94ce32a48b366178ce5863a
```

这张证书来自本机的调试密钥库。**本机它在 `~/.config/.android/debug.keystore`（XDG 路径），不在常见的 `~/.android/debug.keystore`** —— 【实测】后者不存在，前者别名 `androiddebugkey`，SHA-256 与上面 APK 的签名者逐位相同。**换机器/清 HOME 会换签名**，于是「覆盖安装」会失败，见 §3.3。

### 2.4 【实测】本机 `assembleDebug` 失败：缺 Dart SDK

```
cd android && ./gradlew assembleDebug
→ > Task :rust_builder:android:cargokitCargoBuildOpi_ffiDebug FAILED
  /home/wwwroot/bag/opi/android/rust_builder/cargokit/gradle/../run_build_tool.sh: 行 75: dart: 未找到命令
  Process 'command '.../run_build_tool.sh'' finished with non-zero exit value 127
  BUILD FAILED in 57s
EXIT=1
```

成因链（三跳，每跳都有落点）：

1. `app/build.gradle.kts` 依赖 `com.flutter_rust_bridge.rust_lib_app:android:1.0`（`includeBuild("rust_builder")`，见 `android/settings.gradle.kts`）。
2. `plugin.gradle` 把编译 `.so` 挂成 `merge<BuildType>NativeLibs` 的前置任务（`CargoKitBuildTask`），任务体是执行 `cargokit/run_build_tool.sh build-gradle`。
3. `run_build_tool.sh` **本身是个 Dart 启动器**：它先在临时目录生成一个 Dart 包、`dart pub get`、`dart compile kernel`，再 `dart bin/build_tool_runner.dill "$@"` —— **没有 `dart` 命令就走不到 cargo**。报错的行号把这跳钉死了：失败在**第 75 行**，也就是脚本里**第一次**调 `dart`（`"$DART" pub get --no-precompile`），连 pub get 都没跑成。

所以：**要打 APK，必须装 Dart SDK（≥3.0，见 `cargokit/build_tool/pubspec.yaml` 的 `environment.sdk`），或者把 `FLUTTER_ROOT` 指向一个 Flutter SDK**（脚本会用 `$FLUTTER_ROOT/bin/cache/dart-sdk/bin/dart`）。

> **这条值得单独记住**：README 的「构建与测试」一节只写了 `cd android && ./gradlew assembleDebug`，
> **没有列 Dart 这个前提**。而 Dart 在这台机器上**确实用过**：cargokit 的临时目录
> `android/rust_builder/android/build/build/build_tool/` 里留着一份 13MB 的
> `bin/build_tool_runner.dill`，**它只能由 `dart compile kernel` 生成**，且它的 mtime
> 与最后一次成功打出的 APK 是**同一分钟**；同目录还有 `pubspec.lock` 与 `.dart_tool/`。
> 今天 `which dart` 零命中（`FLUTTER_ROOT` 也没有），构建就断在那一跳。

---

## 3. 装到设备

### 3.1 `adb install` 的确切形态

```bash
adb devices                                     # 先确认设备在
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

`-r` = replace，保留应用数据（`filesDir` 里的词库副本与用户词表都留着）。不加 `-r` 且已装同包名，会 `INSTALL_FAILED_ALREADY_EXISTS`。

【实测】本机四种情形的结果（同一台模拟器上，用一个自建的探针包 `io.opi.probe.versioncode` 跑出，用完已卸载；`app-debug.apk` 的安装也真跑过）：

| 情形 | 命令 | 结果 |
|---|---|---|
| 全新安装 | `adb install v5.apk` | `Success`，EXIT=0 |
| 同 versionCode 覆盖 | `adb install -r v5.apk`（机上已是 5） | `Success`，EXIT=0 |
| **降级覆盖** | `adb install -r v3.apk`（机上已是 5） | **失败**：`INSTALL_FAILED_VERSION_DOWNGRADE: Downgrade detected: Update version code 3 is older than current 5`，EXIT=1 |
| 显式允许降级 | `adb install -r -d v3.apk` | `Success`，EXIT=0 |

【实测】对本仓库真 APK：`adb install -r .../app-debug.apk` → `Performing Streamed Install / Success`，EXIT=0。

### 3.2 `versionCode` 必须递增（否则覆盖安装失败）

`android/app/build.gradle.kts` 的 `defaultConfig` 注释原文：

> 与发布 tag 对齐（此前 9 个 tag 期间 versionCode 一直是 1、versionName 一直是 "1.0.0"：APK 既无法被识别，也无法覆盖安装升级）。

对着 §3.1 的实测表读这条注释，**准确的失效形态是「降级」**：

- 新 APK 的 `versionCode` **低于**机上已装的 → 直接拒装（`INSTALL_FAILED_VERSION_DOWNGRADE`）。用户看到的是「安装失败」，且没有可读的原因。
- 新 APK 的 `versionCode` **等于**已装的 → `-r` **能装上**。所以历史上「versionCode 恒为 1」的伤害主要落在「无法被识别」（系统与商店区分不出两个包），而不是「装不上」——**别把这两件事混成一句**。
- 只在临时排障时才用 `-d`（allow downgrade），它不是发布手段。

**发布流程上真正要守的**：`versionCode` 与 `versionName` 在 `android/app/build.gradle.kts` 的 `defaultConfig` 里；`versionName` 是「向根 `Cargo.toml` 的 `[workspace.package] version` 对齐」的那一份（README「技术路线」的「版本」一行）。**本文不写当前值** —— 读法：

```bash
grep -nE 'versionCode|versionName' android/app/build.gradle.kts             # 源码里那份，不需要构建
cat android/app/build/outputs/apk/debug/output-metadata.json                # 已构建过的那份
adb shell dumpsys package io.opi.input | grep -E 'versionCode|versionName'  # 机上装的那份
```

（别用 `./gradlew :app:properties` 找 —— 【实测】它输出的是 `version: unspecified`，**没有** `versionCode` / `versionName`，照着找会以为值没配。）

【实测】第三条在机上给出 `versionCode=9 minSdk=21 targetSdk=34` 与 `versionName=1.0.16` —— 而 `build.gradle.kts` 里**现在写的不是这两个数**，机上那份是 2026-09-27 22:14 装的旧包（`lastUpdateTime` 可查）。这正是「机上版本 ≠ 源码版本」的现场，装机后先对一次这行。

### 3.3 签名不一致会怎样

覆盖安装要求**签名相同**。debug 包的签名来自本机调试密钥库（§2.3，本机在 `~/.config/.android/debug.keystore`）：换一台机器、或清了 HOME/XDG 目录重新生成密钥库之后，同一个 APK 会变成「不同签名的同一个包」，覆盖安装失败（`INSTALL_FAILED_UPDATE_INCOMPATIBLE`）。解法只有先 `adb uninstall io.opi.input`（**会连 `filesDir` 里的词库副本与用户词表一起删掉**）。

---

## 4. 使用

### 4.1 启用输入法

图形路径：**系统设置 → 系统 → 语言和输入法 → 屏幕键盘 → 管理键盘 → 勾选「OPI IME」**，再在「当前键盘」里切过去。路径随 ROM 措辞不同（本项目 `AndroidManifest.xml` 里**一条 `uses-permission` 都没有**，所以不会有权限弹窗挡路）。

命令行路径（【实测】，命令原样可用）：

```bash
adb shell ime list -s -a                              # 列出所有可用输入法，本机输出含 io.opi.input/.OpiImeService
adb shell ime enable io.opi.input/.OpiImeService      # 在「管理键盘」里勾上
adb shell ime set    io.opi.input/.OpiImeService      # 切为当前输入法
```

本机实测输出分别是 `already enabled for user #0` 与 `Input method io.opi.input/.OpiImeService selected for user #0`（两次都 EXIT=0；因为机上本来就是它，所以是幂等的无操作）。

设置页的入口有两个：**启动器图标**（`AndroidManifest.xml` 把 `settings.SettingsActivity` 直接声明为 LAUNCHER，**没有 MainActivity**），以及**输入法自身的设置入口**（`res/xml/method.xml` 的 `android:settingsActivity` 指向同一个 Activity）。所以「设置页」和「输入法」是同一个 App —— 同一个进程、同一份 Rust 静态单例引擎，设置页改的开关在输入法里立刻生效。

### 4.2 字母盘与模式键

- 左下角那个**单字模式键**循环 **中 → 繁 → 英 → 中**（`ImeScreen.kt` 的 `toggleMode`），键面字由 `modeLabelOf` 给出（中/繁/英）。读屏念的是「中英切换」（`keyboard/KeyButton.kt` 的 `spokenKeyName`）。
- 切模式会**清掉未提交的拼音缓冲**（`toggleMode` 里显式 `clear()`，防止半截拼音被空格/回车意外提交成别的词）。
- **`⇧` 只在英文模式可见**（`ImeScreen.kt` 的 `shiftVisible`）。单击 = 下一个字母大写一次，长按 = 大写锁定；离开英文模式会强制复位（`EngineController.resetShift`）。
- 空格：有缓冲 = 选第一个候选；无缓冲 = 直接上屏一个空格（`KeyRouter.handleSpace`）。
- 回车：有缓冲 = 选第一个候选；无缓冲 = 读目标应用声明的 action，**action 位为 0 时提交换行**（`OpiImeService.performEnter`，不硬编码 SEND）。
- 退格：有缓冲 = 退引擎的拼音；无缓冲 = 删编辑框里的字（按**码点**删，emoji 不拆半，`OpiImeService.deleteBackward`）。
- 英文模式且缓冲为空时，字母**根本不经过引擎**，直接上屏（`KeyRouter.handleKey` 的直传分支）。

### 4.3 数字面板与符号面板

**入口是两级**：字母盘底部功能行的 `123` → 数字面板，数字面板底部功能行的 `?123`（`ABC` 右边那个）→ 符号面板。

- 数字面板：键位是 Gboard 风格 —— `1``2``3` / `4``5``6` / `7``8``9` 三行，加一行标点 `,` `0` `.`（`keyboard/NumberPad.kt` 的 `numberRows`）。数字进引擎缓冲，`,` 与 `.` 走**引擎的标点表**（不是 UI 直提），所以中文/全角标点开关在这里同样生效（`ImeScreen.kt` 里 `NumberPad` 的 `onKey = router::handleKey` 注释）。底部功能行最左 `ABC` 切回字母。
- 符号面板：顶部 `ABC`（回字母）/ `123`（回数字）+ 一个**搜索框**；下面三个 Tab：**常用 / 表情 / 全部**（`keyboard/SymbolPanel.kt`，标签表在同文件的 `SymbolTab` 分支里）。
- 搜索：占位文字是「搜索（拼音/英文）」。点搜索框会**在面板下方叠一层字母盘**（IME 窗口内没有系统键盘，所以要自带一个）；输入有 250ms 防抖（`ImeState.SEARCH_DEBOUNCE_MS`）。**这层叠盘的高度是 96dp** —— `ImeScreen.kt` 里有一段长注释说明它为什么不是 176dp（高 density 机型上会把结果区挤成 0dp），**且注明「本机无设备，此改动未实测」**。
- 开任何面板之前，未提交的拼音会**先提交**（有候选选第一个；纯乱码缓冲如 `abc` 清掉）；数字面板的缓冲本身就是待上屏文本，原样提交（`ImeState.commitPendingBuffer`）。

### 4.4 候选栏与翻页 / 长按删词

- 候选栏在字母盘上方**固定预留 44dp**（`candidate/CandidateBar.kt` 的 `CANDIDATE_BAR_HEIGHT_DP`；`ImeScreen.kt` 里写了理由：候选栏出现/消失极其频繁，条件挂载会让键盘高度抖动）。
- **每页 8 个**（`EngineController.pageSize`），左右箭头翻页，页码读作「第 N 页，共 M 页」。
- **单击候选** = 选中并上屏（同时被学习记频）；**长按候选** = 删除这条用户自造词（`EngineController.removeUserWord`；只删用户词表里的条目，删非用户词无副作用）。
- 读屏已接：候选变化自动播报（`liveRegion`），每个键有可读名称。

### 4.5 设置页

`settings/SettingsScreen.kt`，五项：

| 项 | 行为 |
|---|---|
| 学习（开关） | 关掉就不记用户词频；小欧的表情跟着这个开关走（睡着/醒着） |
| 清除用户词库 | 确认对话框 → 清内存里的用户词**并删掉落盘文件**（不删文件的话，输入法下次启动会把它 import 回来，「清除」等于重启后失效） |
| 导出词库 JSON | 复制到剪贴板 |
| 从剪贴板导入 | 与「导出」闭环（复制→粘回来），不经文件管理器 |
| 导入词库 JSON 文件 | 系统文件选择器；MIME 放宽到 `application/json` / `text/plain` / `application/octet-stream`（云盘与 IM 转存的 `.json` 常被判成后两者），有**有界读**上限，超限给可见的失败提示而不是主线程 OOM |

---

## 5. 验证

### 5.1 词库到底装进去没有 —— 两级证据

词库通路是：**APK 里的 `assets/*.opid` → 解压到 `filesDir/` → `OpiEngine.load(path)`**，编排在 `jni/EngineLoader.kt` 的 `load(context)`（`OpiImeService.onCreateInputView` 与 `SettingsActivity.onCreate` 各调一次）。

**第一级：看日志（最省事）**

```bash
adb logcat -d -s EngineLoader:V OpiImeService:V     # -d = 打完就退出；去掉 -d 是持续跟踪
```

装载成功会打出**字节数**；【实测】本机：

```
I EngineLoader: luna loaded (1712082 bytes)
I EngineLoader: trad loaded (2580782 bytes)
```

失败则是另一条 **warning**：`luna load failed, engine on builtin fallback dict`（或 trad 的对应句）。**看到这条 = 词库没装上，退到了内置的回退词库**（`EngineLoader.fallback` → `OpiEngine.load(null)`）。装载失败**不崩 IME** 是设计目标（`loadAsset` 用 `catch (Exception)` + 对 `load` 单独 `catch (Throwable)`）。

**第二级：把日志里的字节数按到 APK 里的资产长度上（决定性）**

```bash
unzip -v android/app/build/outputs/apk/debug/app-debug.apk | grep opid
adb shell run-as io.opi.input ls -la files/          # 调试包才能 run-as
```

【实测】三者对得上：

| 检查 | 值 |
|---|---|
| APK 里 `assets/luna.opid` 的 `Length` | 1712082（`Stored`，即不压缩） |
| `files/luna.opid` 的字节数 | 1712082 |
| 日志 `luna loaded (…) bytes` | 1712082 |

**日志里的数 == APK 里那个资产的 `Length`**，才说明进程真正映射的是这一份词库。三者不一致时，通常是「装的是另一个 APK」（比如旧的 debug 包）—— §3.2 最后那条 `dumpsys` 就是为这个准备的。

### 5.2 幂等：「重拷」到底有没有发生

`EngineLoader.needsCopy(assetSize, existingSize)` 的判据是 `existingSize == null || existingSize != assetSize` —— 尺寸一致就跳过拷贝。这个跳过是**必须**的，代价写在 APK 那一侧：

`android/app/build.gradle.kts` 的 `androidResources.noCompress` 注释原文：

> .opid 词库不压缩存储。压缩后 Assets.openFd() 会抛 IOException，EngineLoader.assetLength() 只能返回 null → needsCopy 恒为 true，于是每次建 IME 视图、每次开设置页都在主线程重拷 3.3MB（luna 1.24MB + trad 2.08MB），尺寸校验的幂等优化被完全废掉。
> 代价：APK 增大约 1.5MB（实测 3.3MB 压缩到 1.76MB）—— 换输入法启动延迟，值。

**机制【实测】**（`noCompress` 确实生效）：APK 里两份 `.opid` 都是 `Stored`、压缩率 `0%`，而三个 `.so` 是 `Defl:N 50%~64%`。**这两份资产是唯一要求「不压缩」的**，`noCompress += setOf("opid")` 只点名后缀。

**拷贝有没有被跳过【实测】**：看 `files/luna.opid` 的 **mtime** 会不会随 IME 启动而变。

```bash
adb shell run-as io.opi.input ls -la --full-time files/
```

本机结果：词库文件 mtime 停在 `2026-09-27 12:20:19`，而同一台机器上 IME 在 `2026-09-28 04:48` 与 `04:50` 各启动过一次（日志有时间戳）。**mtime 没动 = 没有重拷 = 尺寸校验的幂等路径真的走到了。**

> **注释里的字节数已经过期，本文不抄。** 那段注释写的「3.3MB（luna 1.24MB + trad 2.08MB）」是写它那天的实测值；【实测】今天仓库里两份资产**都已经比它大**（luna 那份涨得尤其多）—— 新值请自己读：`stat -c '%n %s' android/app/src/main/assets/*.opid`。**结论不受影响（反而更强：要重拷的字节更多了），但别把那段注释里的数当现值。** 详见 §6.3。

### 5.3 noCompress 有没有生效（一条命令）

```bash
unzip -v android/app/build/outputs/apk/debug/app-debug.apk | grep opid
```

要求：`Method` 列是 **`Stored`**，`Ratio` 列是 **`0%`**。若是 `Defl:N`，说明 `noCompress += setOf("opid")` 没起作用，`assetLength()` 会拿到 `null`，§5.2 的幂等就是废的。

### 5.4 装机状态

```bash
adb shell dumpsys package io.opi.input | grep -E 'versionCode|versionName|primaryCpuAbi|lastUpdateTime'
adb shell settings get secure default_input_method          # 当前输入法
adb shell settings get secure enabled_input_methods         # 已启用的输入法列表
```

【实测】本机分别给出 `versionCode=9 …`、`versionName=1.0.16`、`primaryCpuAbi=x86_64`、以及 `io.opi.input/.OpiImeService` —— **`primaryCpuAbi` 是模拟器/真机与 §1.3 那张 ABI 表的交点**，装错 ABI 的包会在这里露出来（或者是运行时 `UnsatisfiedLinkError`）。

### 5.5 出问题时看哪里

| 现象 | 先看 |
|---|---|
| 键盘出来但不出一字、也无报错 | 日志有没有 `engine on builtin fallback dict`（§5.1） |
| 键盘是空白壳 / 根本不出现 | `OpiImeService` 的 `onCreateInputView` 日志。`onEvaluateInputViewShown()` 被重写为**恒真**（对外接硬件键盘的模拟器是必需的，默认实现在那种设备上返回 false，视图永不创建） |
| 启动即崩、报 `UnsatisfiedLinkError` | `.so` 的 ABI 与设备不符，或 `System.loadLibrary` 失败。注意 `EngineLoader` 的注释明确划了边界：**它只覆盖「词库」层面的失败，`.so` 缺失是另一个失败域，而且发生得更早**（`OpiEngine` 是 object，类初始化里就 loadLibrary） |
| 装了但还是旧词库 | §5.1 第二级 + §3.2 最后那条 `dumpsys` |

---

## 6. 已知限制与未验证项

### 6.1 候选翻页有上限：`fetchLimit` 仍写死 64

**这是端侧限制，不是引擎限制。**

- 现状：`engine/EngineController.kt` 的 `const val fetchLimit = 64`（**以该常量当前值为准** —— 修了它这条限制就没了，本文的 64 只是写作当天从代码里读到的），`refresh()` 里 `api.candidates(fetchLimit)` 一次取这么多条，翻页（每页 `pageSize`）纯客户端做，**引擎侧没有 offset**。
- 引擎侧早就没有这个上限了：`FETCH_LIMIT` 与 `Engine::select` 内部那道截断都已拆掉，三处同值（`engine-core/src/router.rs` · `fcitx5-opi/src/candidate.rs` · `tsf-opi/src/logic.rs`，互相之间**有等值断言**）。【实测】后两处今天都是 `pub const FETCH_LIMIT: usize = usize::MAX;`，紧邻一行就是与 `engine_core::router::FETCH_LIMIT` 的 `assert!`。
- 也就是说：**长前缀拼音打出的候选超过 64 条时，Android 端翻不到第 64 条之后**。macOS 端是同一个病：【实测】`macos/InputController.swift` 调 `engine.candidates(limit: 64)`，而默认值也是 64（`macos/OpiEngine.swift` 的 `candidates(limit: Int = 64)`）。
- 【未验证】「长前缀能不能翻到末页」这件事**在 Android 真机/模拟器上没跑过** —— 这一条是静态取证（读 `EngineController` 的常量 + 引擎侧门禁），不是实测。

### 6.2 本机验证不了、且确实没验过的

| # | 事项 | 依据 |
|---|---|---|
| 1 | **`assembleDebug` 过不去**：缺 Dart SDK（§2.4） | §2.4 实测报错 |
| 2 | `assembleRelease` 从未真正构建过（只跑过 `--dry-run`） | §2.3 |
| 3 | 符号面板搜索态的 96dp 叠盘在**高 density 机型**上的实际观感 | `ImeScreen.kt` 该处注释自称「本机无设备，此改动未实测」 |
| 4 | 横屏下键区占屏高（注释称 57%→68%，未在真横屏机上验） | `OpiImeService.keyboardHeight` 的注释 |
| 5 | 真机（`arm64-v8a` / `armeabi-v7a`）上的任何行为 | §1.2 本机只有 x86_64 模拟器 |
| 6 | 候选翻页上限（§6.1）的真实影响面 | §6.1 |

### 6.3 仓库里正在漂的数字（别照抄）

写本文时顺手核到的、**属于别人文件、本文没有改动**的三处：

| 位置 | 说了什么 | 实测现状 |
|---|---|---|
| `android/app/build.gradle.kts` 的 `androidResources.noCompress` 注释 | 「重拷 3.3MB（luna 1.24MB + trad 2.08MB）」「3.3MB 压缩到 1.76MB」 | 两份资产都已大于注释里的值（`stat -c '%n %s' android/app/src/main/assets/*.opid`）。**结论仍成立、数字已过期** |
| 同文件 `defaultConfig` 的 `versionCode` / `versionName` 与机上的包 | —— | 机上装的是 2026-09-27 22:14 的旧包（`versionCode=9` / `versionName=1.0.16`），与源码**当前值不同**（§3.2） |
| `android/app/build/outputs/apk/debug/app-debug.apk` | —— | 它里面的 `assets/luna.opid` 比仓库里那份 assets **旧**（仓库资产 mtime 晚于该 APK 的构建时间）⇒ 装这个 APK 装到的是**上一版词库** |

这三处都不是本文的活，**报告给 owner**：改法优先「换成读得出来的命令（`stat` / `dumpsys` / `unzip -v`）」，而不是把数换新 —— 换了明年还会漂。

---

## 附：一页速查

```bash
# 编译（注意在 android/ 下）
cd android
./gradlew testDebugUnitTest          # 单测，不需要 Dart
./gradlew assembleDebug              # 打 APK，需要 Dart SDK（§2.4）

# 装机
adb install -r android/app/build/outputs/apk/debug/app-debug.apk

# 启用
adb shell ime enable io.opi.input/.OpiImeService
adb shell ime set    io.opi.input/.OpiImeService

# 验证
adb logcat -d -s EngineLoader:V OpiImeService:V                 # 词库装载字节数
unzip -v android/app/build/outputs/apk/debug/app-debug.apk | grep opid   # 应为 Stored / 0%
adb shell run-as io.opi.input ls -la --full-time files/         # 幂等：mtime 不该变
adb shell dumpsys package io.opi.input | grep versionCode        # 机上版本
```
