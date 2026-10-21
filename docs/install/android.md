<!-- SPDX-FileCopyrightText: 2026 erik.xyz -->
<!-- SPDX-License-Identifier: MIT -->

# Android 端（原生 IME）编译 / 打包 / 安装 / 使用

> ### 先读这一段
>
> 本机有 Android SDK + NDK，还接着一台 **x86_64 模拟器（API 36）** —— `adb` 与
> `./gradlew` 都能真跑，所以本文多数条目是**【实测】**（三个有端代码的平台里，
> Windows 那端本机完全没有条件，见 `docs/install/windows.md`）。
>
> **订正（2026-10-11）**：早前「本机编不了」的硬前提已不成立 —— Dart SDK 已在
> （`/usr/local/bin/dart`，随 Flutter 装上的），`assembleDebug` 与 `assembleRelease`
> **都已真构建并实机运行过**；后者是**分发口径**（debug 构建的 Rust 是未优化版，
> 见 §2.3）。全端性能预算与实测见 [`docs/performance.md`](../performance.md)。
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

`android/app/src/main/assets/` 下七个**入库的构建输入**，Android 的构建流程**不会重新生成它们**（换了 `data/raw` 而不重新编译，APK 里装的还是旧的）：

| 资产 | 内容 | 生成器 |
|---|---|---|
| `luna.opid` | 简体词库 | `scripts/gen_luna_dict.py`（经 `opi-tools compile`） |
| `trad.opid` | 繁体词库（139,355 条；2026-10-18 起含 `trad_modern.tsv` 现代词 42,304 条与康熙差集 622 条） | `scripts/gen_trad_dict.py` + `gen_trad_modern.py` + `gen_kangxi_extra.py` |
| `stroke.opid` | 笔画词典（2026-10-09：笔画键盘 / 生僻字按笔画查字） | `scripts/gen_stroke_dict.py` |
| `rare_index.tsv` | 部首+总笔画索引（生僻字面板） | `scripts/gen_rare_index.py` |
| `wubi86.opid` / `wubi98.opid` | 五笔码表两张（2026-10-14，M4；**按方案二选一装载**，见 `EngineLoader.load` 尾段） | `scripts/gen_wubi_dict.py` |
| `kangxi.tsv` | 康熙字典词库（2026-10-16；**首启种进词库目录一次** —— 之后就是普通导入词库，词库页可停用/删除，删过不再复活）。2026-10-18 起默认库里缺的康熙 (字,读音) 也已并进 `luna.opid` / `trad.opid`（删掉导入词库后仍可打） | `scripts/gen_kangxi_dict.py` + `gen_kangxi_extra.py` |

授权逐条记在 `data/raw/LICENSES.md`（stroke 为 LGPL-3.0、rare 为 Unicode License、
五笔 86 为 LGPL-3.0（与 luna 同族）/ 98 为 Unlicense、kangxi 为 MIT）。

---

## 2. 编译与打包

### 2.1 命令（**`gradlew` 在 `android/` 下，仓库根没有它**）

```bash
cd android                      # 必须。仓库根、desktop/ 的 gradlew 都不是这个工程

./gradlew testDebugUnitTest     # JVM 单测（引擎控制器 / IME 状态机 / 键盘路由 / 用户词存储 / 词库装载 / 宠物）
./gradlew assembleDebug         # 打 debug APK（cargokit 顺带编 opi-ffi 三 ABI .so）
./gradlew assembleRelease       # 打 release APK（**分发口径**：debug 构建的 Rust 未优化、慢 3-20×，见 §2.3）
./gradlew clean                 # 根 build.gradle.kts 里注册的 Delete 任务
```

**发布口径的产物落点 = 仓库根的 `dist/`**（与 Linux 侧 `build-packages.sh` 同一条约定，
见 `docs/install/linux.md` 的「打包（`.deb` / `.rpm`）」一节）：`assembleRelease` 只是把它打在 gradle 的
`app/build/outputs/apk/release/app-release.apk`，**要自己再拷一份**进 `dist/`，
并按发布命名规范改名 —— 三件资产的名字形状一致，`gh release upload` 才不会传错：

```bash
cp android/app/build/outputs/apk/release/app-release.apk \
   dist/opi_<版本>_android-release.apk        # 例：dist/opi_1.9.3_android-release.apk
```

⚠️ 2026-10-15 补：这条此前只存在于发版人的手上，v1.9.2/v1.9.3 两次都是**手工**拷的
（`dist/` 里只有 deb/rpm，用户点名才发现）。APK 的 versionCode/versionName 用
`aapt2 dump badging … | grep ^package` 核（`aapt2` 在 `$ANDROID_HOME/build-tools/<ver>/`）。

【实测】`./gradlew testDebugUnitTest` → `BUILD SUCCESSFUL`，EXIT=0。**它不需要 Dart、也不需要 `.so`**：

- 任务图里**没有** cargokit 那条（`./gradlew testDebugUnitTest --dry-run` → EXIT=0，`grep -c cargokit` 为 **0**）。对照：`./gradlew assembleRelease --dry-run` 的任务图里**有** `:rust_builder:android:cargokitCargoBuildOpi_ffiRelease`（`grep -c cargokit` 为 1）。
- 原因是单测用的是假引擎与假 `FileOps`（`EngineLoader` 的文件操作与加载抽象就是为此抽出来的，见 `android/app/src/test/kotlin/xyz/erik/opi/jni/EngineLoaderTest.kt`）。

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

**订正（2026-10-20）：release 不再用 debug 签名。** `android/app/build.gradle.kts` 里
`buildTypes.release` 现在指向 `signingConfigs.release`，密钥库是仓库内自带的
[`android/keystore/`](../../android/keystore/README.md)（`opi-community.jks`）
（公开的 sideload 密钥，密码与警告见该 README）。固定它之后，任何机器、任何 CI run
打出的 release APK 都是同一张证书，可以直接互相覆盖安装。

以下三条是旧状态（release = debug 签名）的记录，第 3 条仍然有效：

1. ~~**release 用 debug 签名**~~（已订正，见上）。旧后果：release APK 与本机 debug APK
   同签名，但 debug 签名来自每台机器各自生成的 `~/.android/debug.keystore` ——
   **换机器、或 CI 每次新生成密钥库，证书都会变**，同一个包名的新旧 APK 无法覆盖安装
   （`INSTALL_FAILED_UPDATE_INCOMPATIBLE`）。1.17.0 → 1.17.1 的用户实测正是这个现象。
2. 那条注释里的**理由措辞是 Flutter 时期的**（`android/` 下早已没有 flutter 目录，Flutter 方案随 M6a 原生重构作废，见 `docs/superpowers/plans/2026-08-12-opi-ime-android-m4.md` 头部的订正注记）。
3. **订正（2026-10-11）：release 变体已实测构建多次**（`BUILD SUCCESSFUL`，并实机安装运行），且成为 dist 的**分发口径** —— ⚠️ 关键差别：**`assembleDebug` 的 Rust 是 debug 编译**（cargokit 的 `builder.dart` 只在非 debug 时加 `--release`），同一按键实测 **46–239ms（debug） vs 23–83ms（release）**，3–20×。首次构建要拉 `lint-gradle` 工件，撞阿里云镜像 TLS 抖动**重试即可**。出处与完整实测表：[`docs/performance.md`](../performance.md)。

release APK 的签名（可随时用下面的命令复核，CI 里也钉了这个指纹）：

```
apksigner verify --print-certs android/app/build/outputs/apk/release/app-release.apk
  → Signer #1 certificate DN: CN=opi community, O=opi, C=CN
    SHA-256: 3fe2867ce790f5e472cf16381264eb69b2807c7e58256678a6cef70a3dcf00bf
```

`assembleDebug` 仍用本机 debug 密钥库（仅本地调试，**勿分发**）。本机它在
`~/.config/.android/debug.keystore`（XDG 路径），不在常见的 `~/.android/debug.keystore`
—— 【实测】后者不存在，前者别名 `androiddebugkey`。这部分只影响 debug 包。

### 2.4 【已订正】本机 `assembleDebug` 失败：缺 Dart SDK（2026-08-14 记录；今已满足）

> **订正（2026-10-11）**：Dart 已在（`which dart` → `/usr/local/bin/dart`），本节的失败
> **不再复现** —— debug/release 均实测构建通过。成因链保留作诊断记录（哪天换机器又缺
> Dart，症状与下文逐字相同）。

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
adb shell dumpsys package xyz.erik.opi | grep -E 'versionCode|versionName'  # 机上装的那份
```

（别用 `./gradlew :app:properties` 找 —— 【实测】它输出的是 `version: unspecified`，**没有** `versionCode` / `versionName`，照着找会以为值没配。）

【实测】第三条在机上给出 `versionCode=9 minSdk=21 targetSdk=34` 与 `versionName=1.0.16` —— 而 `build.gradle.kts` 里**现在写的不是这两个数**，机上那份是 2026-09-27 22:14 装的旧包（`lastUpdateTime` 可查）。这正是「机上版本 ≠ 源码版本」的现场，装机后先对一次这行。

### 3.3 签名不一致会怎样

覆盖安装要求**签名相同**。自 1.18.0 起分发包统一用仓库内的社区签名密钥库
（§2.3、[`android/keystore/README.md`](../../android/keystore/README.md)），
之后所有版本证书相同，可直接覆盖升级。

**从旧版切过来仍需卸载一次**：1.17.1 及更早的发布包是构建机各自的 debug 签名，
与这张证书不同，Android 不允许「同包名、不同证书」覆盖安装
（`INSTALL_FAILED_UPDATE_INCOMPATIBLE`）。用户必须先 `adb uninstall xyz.erik.opi`
（**会连 `filesDir` 里的词库副本与用户词表一起删掉**），再装 1.18.0。
这是签名机制的一次性代价，不是 bug：换任何签名密钥都一样。

`assembleDebug` 出来的调试包仍用本机 debug 密钥库，**与分发包互不覆盖**，只用于本地调试。

### 3.4 覆盖安装后的自动重载（2026-10-18）

**行为**（`PackageReplacedReceiver` + manifest 里 `exported="false"` 的 `MY_PACKAGE_REPLACED` 接收器）：

1. 覆盖安装完成、系统投递 `ACTION_MY_PACKAGE_REPLACED` 后，接收器让本进程所有已装载
   的用户词 store 把防抖窗口里的保存立即落盘（`UserWordStore.flushAll` →
   `UserWordStore.flushPending`）。**未装载过用户词时什么都不写** —— 那时引擎里没有
   导入过文件，导出的是空表，写回等于清空用户词。
2. 延迟 `UserWordStore.SAVE_DEBOUNCE_MS + 500ms`（写作时为 2 秒）后调用
   `Process.killProcess(Process.myPid())` 结束进程。旧进程里已 `System.loadLibrary`
   的 `.so` **无法卸载** —— 清 `inputViewCache` 或重载词库换的只是数据，跑的还是旧
   机器码；所以能证明有效的最小方案只能是结束进程。
3. 系统下次需要键盘时（点输入框，或当前输入框重新请求输入法）以新 APK 重新绑定
   `OpiImeService`，新进程装载新代码与新 `.so`。

**不需要用户操作的情况**：覆盖安装时系统自己杀掉了旧进程（常见路径）—— 接收器在新
进程里跑一遍、额外结束一次空进程，无副作用；下次弹键盘就是新版。

**仍需用户操作 / 做不到的情况**（如实列出）：

- **首次从「还不含本接收器」的旧版本升上来**：旧 APK 里没有这个类，若旧进程恰好被
  系统保留，这一次兜不住（切一次输入法或重启手机即可）。之后的版本升级才有兜底。
- **系统不投递或延迟投递 `MY_PACKAGE_REPLACED`**：部分 OEM 的后台/自启动策略会拦
  隐式广播，表现就是回到旧行为（要手动重启）。【未验证】具体 ROM 名单 —— 本机只有
  x86_64 模拟器，**不声称所有机器都会自动重启**。
- **更新发生在正在打字时**：未上屏的组合串（拼音缓冲）会随进程消失；已提交的文字、
  已落盘的用户词与偏好不受影响。键盘可能短暂消失，点一下输入框（或等系统重绑）即可
  回来 —— 重绑时机随 ROM 不同，慢的机型需要手动点一次。

**核对一次更新是否真的生效**：

```bash
adb logcat -s PackageReplaced:I OpiImeService:I   # 覆盖安装后应有 MY_PACKAGE_REPLACED 一行
adb shell pidof xyz.erik.opi                      # 安装前后的 pid 应当变化
```

【未验证】本机没跑过「覆盖安装 → 自动 kill → 键盘自动回来」的完整链路。接收器判定、
落盘时机由 JVM 单测钉住（`PackageReplacedConfigTest`、`UserWordStoreTest` 的
`flush*` 用例），**系统侧的重绑行为只有在真机上才能确认**。

---

## 4. 使用

### 4.1 启用输入法

图形路径：**系统设置 → 系统 → 语言和输入法 → 屏幕键盘 → 管理键盘 → 勾选「小欧输入法」**，再在「当前键盘」里切过去。路径随 ROM 措辞不同（本项目 `AndroidManifest.xml` 里**一条 `uses-permission` 都没有**，所以不会有权限弹窗挡路）。

命令行路径（【实测】，命令原样可用）：

```bash
adb shell ime list -s -a                              # 列出所有可用输入法，本机输出含 xyz.erik.opi/.OpiImeService
adb shell ime enable xyz.erik.opi/.OpiImeService      # 在「管理键盘」里勾上
adb shell ime set    xyz.erik.opi/.OpiImeService      # 切为当前输入法
```

本机实测输出分别是 `already enabled for user #0` 与 `Input method xyz.erik.opi/.OpiImeService selected for user #0`（两次都 EXIT=0；因为机上本来就是它，所以是幂等的无操作）。

设置页的入口有两个：**启动器图标**（`AndroidManifest.xml` 把 `settings.SettingsActivity` 直接声明为 LAUNCHER，**没有 MainActivity**），以及**输入法自身的设置入口**（`res/xml/method.xml` 的 `android:settingsActivity` 指向同一个 Activity）。所以「设置页」和「输入法」是同一个 App —— 同一个进程、同一份 Rust 静态单例引擎，设置页改的开关在输入法里立刻生效。

### 4.2 字母盘与模式键

- 左下角那个**单字模式键**循环 **中 → 繁 → 英 → 中**（`ImeState.cycleMode`），键面字由 `modeKeyLabelOf` 给出 —— ⚠️ **2026-10-12 晚裁决改成「点它会切到哪」**（此前给的是**当前**模式）：用户实测报「切到英文键盘后键面还写着英，点下去却是中文键盘，键面该是『中』才对」。于是中→繁 / 繁→英 / 英→中（回上次的中文方案，若上次是繁体则→繁）；键面只有一个字，读屏按同一语义念成「切到X」（`keyboard/KeyButton.kt` 的 `spokenKeyName` + `stateDescription`）。**改 `cycleMode` 次序必须同改这里**（门禁：`KeyLabelTest.modeKeyLabelShowsTheTargetNotTheCurrentMode`）。
  ⚠️ **2026-10-15 两裁订正**：上面这个中→繁→英循环**只剩 26 键一盘**（搜索叠盘里那枚仍接 `cycleMode`，那处只为穷尽分支存在）。①「简繁按钮就只切换简繁」——**9 键**这枚键改走 `ImeState.toggleTraditional`；②同日「统一成只切简繁」——**14 键与笔画**也归到同一条。这两盘原先**没有**独立的英文入口（进英文全靠模式键循环到「英」那档）⇒ 同一裁决给两盘各补了一枚底行 [英文] 键（接 `state::switchToEnglishKeyboard`，与 9 键同款；2026-10-15 实测全仓只有 9 键底行与 `cycleMode` 两条进英文的路，缺了这枚英文在这两盘不可达）。三盘都只在 **简 ⇄ 繁**（`PINYIN ⇄ TRADITIONAL`）之间切，键面语义不变（仍是「点它会切到哪」）：拼音态「繁」/繁体态「中」，由纯函数 `jianFanModeKeyLabelOf` 给出；读屏名「简繁切换」（`KeyButton` 的 `description` 覆写 —— `spokenKeyName` 对「中/繁」的默认读法「中英切换」会念错动作）、`stateDescription` 复用 `modeStateDescription`（「中文/繁体」）。缓冲语义与中→繁那条腿一致（`switchLanguage`：方案不动、缓冲保留）。门禁：`KeyLabelTest.modeKeyOnlyExpressesChineseAndTraditional` + `threeKeyboardsBindThePlainJianFanToggleAndKeepAnEnglishEntry`（接线静态门禁：三盘切简繁、26 键与叠盘仍 cycleMode、三盘各有独立英文键）+ `ImeStateModeKeyTest.nineKeyModeKeyOnlyTogglesTraditional`（该用例 2026-10-15 随本裁决与两条 cycleMode 用例一起拆到 `ImeStateModeKeyTest.kt` —— 行数门禁）。
- 2026-10-09 起这个循环与**输入模式**接线对齐（键位不变，语义更准）：中→繁**只切引擎模式**（9 键仍是 9 键）；繁→英走英文输入模式（方案回全键盘 —— 9 键上打英文没有候选路），且 **pending 按拼音原样上屏、不挑候选**（2026-10-10 裁决「切英文默认是拼音，不要是候选词」——三处英文入口统一：本循环 / 9 键底行 [英文] / 输入模式面板的英文26键）；英→中回到**上次的中文方案**（如 9 键）。
  **同日二期：三条腿全部保留缓冲**（用户裁决「中文输入的时候切换英文，拼音保留可继续使用」）—— 走 `EngineController.switchLanguage`（引擎侧换模式/方案但**不清缓冲**，且成对切换保证缓冲在两头语义一致），不再提交 pending。英文模式下字母照常**直传**（`KeyRouter`：缓冲非空也直传 —— 保留的拼音绝不被英文打字喂动）；切回中文，打到一半的拼音连同候选立即回来，可接着打/接着选。⚠️ 「输入模式」面板那条仍先提交再换（明确换布局的动作，判据不同）。
  ⚠️ **2026-10-19 订正**：英→中那条腿不再是「保留缓冲」——用户裁决「输入重置」：切换前清空英文缓冲、**不提交**英文原文，切完候选为空（不显示「无匹配」）；中→繁、繁→英两腿仍保留缓冲。门禁：`ImeStateEnglishSwitchTest.cycleFromEnglishToChineseResetsBufferWithoutCommitting`。
- **`⇧` 只在英文模式可见**（`ImeScreen.kt` 的 `shiftVisible`）。**单击 = 锁定大写，再点一次取消**（2026-10-15 用户裁决「点击了就要锁定大写，再次点击取消大写」）；**长按 = 单次大写**（下一个字母大写后自动复位）——这是同一条裁决的下半段：三态（OFF/SINGLE/LOCK）一个没少，只是「单次」与「锁定」换了手势（`EngineController.shiftTap` / `shiftLongPress`，门禁 `EngineControllerTest` / `KeyRouterTest`）。离开英文模式会强制复位（`EngineController.resetShift`）。**2026-10-15 补**：⇧ 激活（SINGLE/LOCK）时**字母键面也显示大写**（`QwertyKeyboard.kt`，用户报障「点击了，键盘还是显示小写，实际输出是大写」）—— 只改**显示**，`onKey` 送出去的仍是原字符，大小写由 shift 语义/引擎定；读屏名跟着键面走（「Q 键」）。9/14 键不涉及（英文模式恒 26 键布局，两盘只在中文方案下渲染 ⇒ 那一侧没有 ⇧）。
- 空格：有缓冲 = 选第一个候选；无缓冲 = 直接上屏一个空格（`KeyRouter.handleSpace`）。
- 回车：有缓冲 = 选第一个候选；无缓冲 = 读目标应用声明的 action，**action 位为 0 时提交换行**（`OpiImeService.performEnter`，不硬编码 SEND）。
- 退格：有缓冲 = 退引擎的拼音；无缓冲 = 删编辑框里的字（按**码点**删，emoji 不拆半，`OpiImeService.deleteBackward`）。
- 英文模式：**默认普通** = 字母直出（`KeyRouter` 不进引擎）；点 `pro` 切专业后才进引擎做单词前缀联想（2026-10-18 起；单字母缓冲的首候选 = 输入原文，保留大小写）。只有**携带缓冲**（中→英保留的拼音）时字母直传。
- **英文 26 键顶部有 0-9 数字行**（2026-10-19，`QwertyKeyboard.showNumberRow`；只在主键盘英文模式显示，搜索叠盘与中文模式不显示）。数字**直接上屏**（`KeyRouter.handleKey` 的英文分支；全角规则同其它直传点），不进引擎的英文数字选词。
- **英文底行布局（2026-10-19）**：`!?#`（进符号页，英文符号在 ASCII 分类）→ `pro`（普通/专业）→ 空格 → 中/英切换 → ⌫；**回车与大写锁定同一行，回车在最右**（第 3 行 `⇧ z…m ↵`），底行不再放回车。原 [123] 位置改为 `pro`（数字已由顶部 0-9 覆盖；符号/数字面板仍从中文键盘进入）。**默认普通** = 字母直出、不查 `en_words.tsv`、无候选；点 `pro` 切到专业 = 现有前缀联想/候选。切换清空英文缓冲并落盘（`ImePreferences.englishPro`），键面高亮表示专业态；`KeyRouter` 的 `englishPro` 判据只在英文模式生效。

### 4.3 数字面板与符号面板

**入口是两级**：字母盘底部功能行的 `123` → 数字面板，数字面板底部功能行的 `?123`（`ABC` 右边那个）→ 符号面板。

- 数字面板：键位是 Gboard 风格 —— `1``2``3` / `4``5``6` / `7``8``9` 三行，加一行标点 `,` `0` `.`（`keyboard/NumberPad.kt` 的 `numberRows`）。数字**直提**（点一下立即上屏，2026-10-10 修复「点了没反应」），`,` 与 `.` 走**引擎的标点表**（不是 UI 直提），所以中文标点开关在这里同样生效；**全角开时数字与标点都出全角**（数字在直提点宽化，见下「全角」段）。底部功能行最左 `ABC` 切回字母。
- 符号面板：顶部 `ABC`（回字母）/ `123`（回数字）+ 一个**搜索框**；下面两个 Tab：**常用 / 全部**（`keyboard/SymbolPanel.kt` 的 `SymbolPageTab`）。表情不占符号页 Tab —— 所有表情入口都走独立的 `EmojiPanel`（见下「表情页」）。
- 搜索：占位文字是「搜索（拼音/英文）」。点搜索框会**在面板下方叠一层字母盘**（IME 窗口内没有系统键盘，所以要自带一个）；输入有 250ms 防抖（`ImeState.SEARCH_DEBOUNCE_MS`）。**这层叠盘的高度是 96dp** —— `ImeScreen.kt` 里有一段长注释说明它为什么不是 176dp（高 density 机型上会把结果区挤成 0dp），**且注明「本机无设备，此改动未实测」**。
- 开任何面板之前，未提交的拼音会**先提交**（有候选选第一个；纯乱码缓冲如 `abc` 清掉）；数字面板的缓冲本身就是待上屏文本，原样提交（`ImeState.commitPendingBuffer`）。
  ⚠️ 例外：**切到英文**（循环腿/底行[英文]/面板英文26键）按**拼音原样上屏**，不挑候选
  —— 2026-10-10 深夜用户裁决，见上一条。

### 4.4 候选区：共用格 · 折叠行 ⌄ · 平铺态（2026-10-16 改版）

- **默认界面全覆盖**（2026-10-17 用户裁决「红线区域内的界面其他视图都覆盖了，只有默认
  界面没有」）：根列铺 `toolbarBg` —— 拼音行空槽（38dp）此前透明会让窗口顶部露一条白带，
  现在与工具条连成一片（布局/高度一字未动；其余视图本来就是整窗 `panelBg` 铺满）。
- **顶部共用槽位：拼音行 | 小欧宠物条**（2026-10-17 裁决链：先「默认顶部好空」→ 短暂
  做过「最近上屏常用语」快捷条 → 同日改成四季花纹边框 → 用户看过实物再裁决「动态花纹
  不好看，移除，对应的功能也移除，改成项目宠物的多形态：笑、欢乐、哭、沮丧、惊恐、愤怒，
  要是动态的、有色彩感」）：
  那条 38dp 槽位**输入中画拼音行**（`ComposingBar`，原样）、**空闲画小欧宠物条**
  （`ime/PetStrip.kt`：六形态按 笑 → 欢乐 → 哭 → 沮丧 → 惊恐 → 愤怒 轮播，每形态停留
  2.8s、0.42s 叠化；小欧本体按情绪轻晃 / 蹦跳 / 发抖 / 垂头 / 急颤 / 横抖，条底用当前
  形态的情绪色做低透明洗色 + 漂浮粒子，深色皮肤自动换深色版小欧；纯装饰、读屏跳过）。
  四季花纹与季节锚点那套（`SeasonBand` / `SeasonTheme` / 隐藏时重校准）**整套撤除**。
  六形态的脸画在共享的 `shared/pet/OpiPet.kt`（`PetMood` 末尾追加 CRYING / TERRIFIED /
  ANGRY —— 跨端枚举仍是只加末尾，鸿蒙草案镜像同步追加）；门禁 `PetStripTest`（次序 /
  包裹 / 枚举序号）+ 离屏渲染探针 `OpiPetFormsRenderProbeTest`（六张脸互不相同，
  哭蓝 / 怒红 / 欢乐粉）。设计对照图见 [`docs/opi-pet-forms.svg`](../opi-pet-forms.svg)
  （六格，与 `docs/opi-pet.svg` 同一套 240×250 几何）。
- **整块键盘隐藏即重置**（2026-10-17 用户裁决「隐藏整个键盘的时候，就重置到默认界面，
  再打开就默认界面」）：`onWindowHidden → ImeState.resetToDefaultView()`（回 QWERTY、
  清搜索、面板模式回字母；**不动**引擎缓冲与学习 —— 那归 `onEditorChanged`（真正结束
  输入）管）。门禁 `ImeStateTest.hideKeyboardResetsViewSearchAndModeToDefaults`。
- **逐字确认 / 延迟提交**（2026-10-17 用户裁决「输入一个词组，匹配到的在最前面；到没有
  匹配，就要先匹配前面的第一个字，匹配到所有读音的字，确认一个字，依然在候选栏；然后
  依次匹配后面的字，直到最后的字匹配确认后，才是最后确认到输入框。可参考同类输入法」）：
  整串有词组精确匹配 ⇒ 词组在最前、选中一次提交整词；整串无精确匹配（前缀扩展不算）
  ⇒ 分词回退档先出**第一个字的所有读音的字**，确认一个字后它**留在候选区**（拼音行显示
  「中guo」，不写输入框），缓冲只剩后缀接着编；依次确认到最后一段 ⇒ **整串一次性提交**。⌫：缓冲非空先退缓冲，
  缓冲空再按 ⇒ 撤销上一个已确认的字（拼音回到缓冲可重选）；空格/回车在逐字态 = 收尾提交。
  语义全在引擎（四端同一条路：`Engine::pending_text` / `display_buffer` /
  `commit_buffer_after_select`）；门禁 `tests/segment_commit.rs`（引擎）+ 9 键点号换算
  `EngineControllerComposingTest.confirmedCharsDoNotInflateTheNineKeyDotCount`。
- **候选栏替换工具条，英文空闲例外**（2026-10-20 用户订正）：候选/工具条仍是**共用一格**，
  有候选时候选栏替换工具条；但 `candidatesShowable` 对英文恒真 ⇒ 英文空闲会永远看不到
  工具条，故加 `englishIdle`（英文且 buffer/candidates 都空）特判，此时显示工具条。中文空闲
  本来显示工具条；中英文只要有输入或候选，候选栏立刻替换工具条。拼音行仍在共用格上方。
- **折叠行**（`candidate/CandidateBar.kt`）：候选大字横排、可横向滑动。**候选项前不显示序号**
  （2026-10-17 用户裁决「移动端候选词不要前面的数字」——2026-10-09 曾恢复过一版，今为终态；
  序号是「按数字选词」的语义，中文键盘没有数字行；英文 26 键自 2026-10-19 起顶部有
  0-9 且数字**直接上屏**（不选词），桌面两端照旧带序号）。
  **右端一枚 ⌄**（2026-10-16 裁决「左右按钮换成 v 的图标」——原「‹ n/m › 左右翻页」整段下线）。
  候选栏不再放「收起键盘」；英文态只多一枚低存在感隐藏「中」键（切回中文）。
  2026-10-21 用户裁决：候选态占满整条工具条，最右不留收起键；收起入口回到工具条。
  （2026-10-20：英文有输入/候选时走候选栏，英文空闲改走工具条，候选栏与工具条两处
  都提供这枚键）。
- **平铺态**（点 ⌄，`ime/CandidateTiledView.kt`）：占满共用格 + 键盘区 —— 左列放大读音列
  （点行仍 = 钉读音；与键盘左列**同一块** `CandidateReadingColumn`）、中间候选**按词长平铺**
  （短词一行 4 个、长词一行 2 个；分页规则是纯函数 `TilePack.kt`）、右列 `返回 / ⌫ / 重输 / 空格`；
  **上下滑动翻页**。收拢：`返回` = 回键盘不动缓冲；选词 / ⌫ 退到空**自动收拢**（否则键盘回不来）。
  状态机门禁 `ImeStateTiledTest`、分页门禁 `TilePackTest`，界面以模拟器截屏核验（2026-10-16 实测七项）。
- **单击候选** = 选中并上屏（同时被学习记频）；**长按候选** = 删除这条用户自造词（`EngineController.removeUserWord`；只删用户词表里的条目，删非用户词无副作用）。
- 读屏已接：候选变化自动播报（`liveRegion`），每个键有可读名称；候选项播报**只念词本身**
  （2026-10-17 随可见序号一同去掉「第 N 项」——「念的与看得见的一致」这条纪律不变，
  变的只是看得见的东西；理由写在 `CandidateBar.kt` 的注释里）。

### 4.5 输入模式 / 键盘模式 / 工具条（2026-10-09，对齐主流移动输入法）

**候选与工具条共用一格**（2026-10-16 改版，见 §4.4）：输入中画候选、其余时候画
工具条；格位常驻不伸缩。工具条（`ime/ImeToolbar.kt`，高 = `IME_TOOLBAR_HEIGHT_DP`，计入
IME 窗口列布局）**最左是小欧（logo）**，其后才是入口图标 —— 2026-10-09 用户裁决
「入口要和 logo 图标在一行，放在 logo 后面」。
入口**全部用图标**（自绘矢量，`keyboard/OpiIcons.kt` —— 不引
`material-icons-extended` 整包，理由在文件头）：

| 图标 | 入口 |
|---|---|
| 笑脸 | **表情**：直接进独立表情页 `keyboard/EmojiPanel.kt`（`ImeState.openEmoji` → `SymbolTab.Emoji` 入口标记） |
| 调色板 | **皮肤**面板：十张预览卡（见下「皮肤」段；2026-10-09 加，2026-10-10 扩到十款） |
| 键盘 | **输入模式**面板：七选一（见下表）。2026-10-09 起与皮肤同一形态的**面板**（占键区、左上角返回箭头单独一行、标题在下一行），不再用 DropdownMenu 弹框（用户裁决「做成像选择皮肤的功能」）；选中即换并自动收尾回键面/生僻字面板 |
| 滑杆 | **键盘模式**面板：繁体 / 单手（关/左/右）/ 悬浮 / 游戏 / 暗光 / 长辈。同上为面板形态；**点选项不关面板**（常要连开两三个），✓ 随开关实时更新，暗光这类换肤当场生效 |
| 圆圈 i | **关于**面板（2026-10-15 用户裁决「关于给移动端键盘功能区加个入口」）：简介 / 版本（`BuildConfig.VERSION_NAME`）/ 版权 / 表情数据（OpenMoji CC BY-SA 4.0、颜文字 MIT）/ 项目地址 / 博客地址，两条地址可点（`ACTION_VIEW`；IME 是 Service，起 Activity 必须 `NEW_TASK`）。文案与设置页那一节**同一份常量**（`settings/AboutSection.kt` 的 `ABOUT_INTRO` / `ABOUT_PROJECT_URL` / `ABOUT_BLOG_URL`），面板里一字不自编；面板**自己可滚动**（键区矮，内容比它高）。门禁 `AboutSectionTest.aboutPanelReusesSourcesAndIsWiredToTheToolbar` |

（2026-10-13 用户裁决「移动端功能区不需要全角、半角切换功能」：本表原「月相（满/半）
→ 全角 ⇄ 半角」一行已移除，图标与入口一并撤掉 —— **引擎能力保留**，见下文「全角」段。）

⚠️ 图标一律**线稿**（`fill = null` + 描边）：首版有一条路径写了填充，闭环子路径全变
实心块（笑脸成圆饼、键盘成黑方块）—— 渲染复核才看得见，已修并由 `OpiIconsTest`
按「每条路径 fill 必须为 null」钉住（阴性对照验过有牙）。

**全角 / 半角**（2026-10-12 用户裁决「字母、数字也要」+「补 Android 入口」）：
- 语义：全角开 ⇒ **ASCII 可打印字符**（字母/数字/标点）平移 +0xFEE0 出全角；
  **空格不宽化**（有意：全角在拼音模式默认开，宽空格会把「按一下空格」悄悄换成
  U+3000），汉字等非 ASCII 原样。默认值随模式：拼音/繁体=全角、英文/数字/符号=半角，
  **切模式重置**（引擎 `Mode::default_fullwidth`）。
- 落点：拼音/繁体模式下字母数字是**输入码**（进缓冲），**绝不宽化**；
  英文/数字模式下它们是**待上屏文本**，但**各端都是直传、不进引擎** —— 所以宽化在
  **宿主直传点**做：Android 是 `KeyRouter` 的英文直传与数字直提两处，走
  `EngineController.fullwidthize`（规则镜像 engine-core 的 `Engine::fullwidthize`，
  锁同步门禁 `FullwidthTextTest` 现读 `punctuation.rs`）。桌面两轨在同一位置各有一处
  （`input_method.rs` / `logic_input_method.rs`，逐行同构）。
- 入口：**无**（2026-10-13 用户裁决「移动端功能区不需要全角、半角切换功能」——工具条
  月相按钮已移除；能力本体与模式默认值原样保留，`EngineController.fullwidthize` 的宽化
  照旧，只是用户不能再手动切）。无持久设置项（它有模式默认值、切模式重置，与「中文标点」
  那种用户偏好不同档）。

**表情页的内容（2026-10-18 起来自 OpenMoji）**：`emojiSymbols()` 给的是
`data/raw/emoji.tsv`（由 `scripts/gen_emoji_kaomoji.py` 从 OpenMoji 裁剪、产物入库、
编译期嵌入；数据来源与 SHA-256 钉在脚本头）。裁剪口径见脚本头注：`flags` /
`component` / `extras-*` 整组不要，肤色变体与含 ZWJ 的组合序列不要，其余按子类设
上限 —— 目标是常见表情，不是全量；组名写在第 3 列（`EMOJI_GROUP_CN`：笑脸→人物→
动物→植物→食物→旅行→活动→物品→符号），引擎按**连续段**切组。⚠️ symbols.tsv 的
emoji 列**不动** —— 它管单键候选反馈（搜索路），与浏览路是两条（见 `symbols.rs`
的 `builtin_emoji_texts` 注释）。

**表情页的布局**（2026-10-18 用户参考布局：顶部 Tab、中间网格、**底部**分类条；
所有表情入口直达本页；符号面板只有 常用/全部，不再保留旧表情网格）：
- **顶部** `[← 返回][自有 | Emoji | 颜文字][🔍 搜索]`（`keyboard/EmojiPanel.kt` 的
  `EmojiTopBar`）：三个子页；返回 = 搜索态退回浏览态并清查询，浏览态关闭面板回键盘。
  自有页只有图片，**没有分类条也没有搜索**；搜索态仍变 `[← 返回][搜索框]` +
  面板下方**叠一层字母盘**（既有机制；防抖在 `ImeState`）。
- **自有（独立顶级子页，2026-10-18 用户裁决）**：12 个小欧形态（贴纸源
  `shared/pet/stickers/` → APK assets `pet/`，文件名契约 `PetStickerAssets`）以
  6 列网格播 GIF（API 28+ 走 `ImageDecoder`，API 21–27 走 `Movie` 兜底；静态 PNG/
  矢量为缺图兜底；`keyboard/PetStickerPanel.kt`）。点一格 ⇒
  `OpiImeService.commitPetSticker`：PNG（其次 GIF）拷到 `cacheDir/pet/`，经
  `FileProvider`（authority `${applicationId}.fileprovider`，`res/xml/file_paths.xml`
  只开 `cacheDir/pet`）构造 `InputContentInfoCompat`，用
  `InputConnectionCompat.commitContent` 插入**图片**（2026-10-20 起 PNG 优先）。编辑器
  两种图片都不接受时先把图（仍 PNG 优先）写进系统剪贴板并提示长按粘贴，**同时**上屏
  **带「小欧·」前缀的可区分文本标签**（如 `[小欧·笑]`），**不再插入
  Unicode emoji**（2026-10-19 用户裁决）。自有不进 Emoji
  分类条、也不进 Emoji 的最近使用。
- **Emoji 子页**：底部一条**可横滑**的分类条（`EmojiPanel.kt` 的 `CategoryBar`），
  内容 = 引擎 `emojiGroups()` 的 **9 组**（不再含自有或任何偏移）；点组
  `scrollToItem(start)`，高亮 = 网格当前首个可见项所属组（`SymbolCatalog.groupIndexOf`）。
  组名/组序来自 `data/raw/emoji.tsv`（OpenMoji 689 条），`start` 与
  `emojiSymbols()` 下标对齐，由 `emoji_coverage.rs` 连续覆盖门禁钉住。
- **颜文字子页**：分类 = `assets/kaomoji.tsv` 的第二列（390 条/7 类；解析在
  `keyboard/KaomojiCategories.kt`：坏行跳过、同名分类并进第一处，`start` 是拼平后
  的累计下标）；缺失/损坏 ⇒ 空列表 + 「没有颜文字数据」空态（不崩 IME）。
- **渲染前保序去重**（同日用户裁决「表情存在重复的，去重」）：`filter`/浏览列表在
  进网格前各过一道 `distinct()` —— 数据/引擎/搜索三层都已各自去重，这是最后一道。
- **渲染前过一道字形可用性**（2026-10-12 晚用户报障「有个表情显示是长方形框」）：
  那是 U+1FAEB「cracking face」（Unicode 17.0），设备系统字体没这个字形 ⇒ 空心矩形。
  数据没错（新机型会显示），所以过滤只在**面板层**：`SymbolCatalog(canRender = ...)`
  注入 `SymbolCatalog.canRenderGlyph`（`Paint.hasGlyph`），**只判单码位**——
  多码位序列（旗帜/家庭/键帽）在 `hasGlyph` 下问的是整串连字、会误判成没有，
  把能显示的整类藏掉比留一个空心框更坏。分类条的下标/条数**跟着重算**
  （`start`/`count` 一起过前缀存活表），否则点分类会滚错组。

**输入模式（九种）**——引擎轴是「方案 × 语言」，布局轴在 UI（双拼与五笔分别于
2026-10-13 / 2026-10-14 加入）：

| 菜单项 | 引擎（`InputScheme` × `EngineMode`） | 说明 |
|---|---|---|
| 拼音26键 | FULL × PINYIN | 既有字母盘 |
| 英文26键 | FULL × ENGLISH | 既有英文盘（⇧ 只在它可见） |
| 拼音9键 | NINE × PINYIN | T9：`2`–`9` 是**输入键**（引擎按键类展开），有候选时数字也不选词；**主区域第一格 2026-10-15 起是分词键**（面「分词」、发 `'` 往缓冲插音节边界，空缓冲由宿主拦住；此前是面「，。?!」、发半角 `,` 走引擎标点表的多标点键——标点现在只在左列）；**另有左右两列**与特有底行（2026-10-10 重排，见下）；底行含**模式键（9 键上只切简/繁，2026-10-15 裁决，见 §4.2）**、**数字/英文键盘切换**与**重输键**（重输 2026-10-10 加入、在右列；「无匹配时分词匹配自动回退」自 2026-10-15 起与手动分词键并存）|
| 拼音14键 | FOURTEEN × PINYIN | `qw er ty ui op / as df gh jk l / zx cv bn m`，发送键 = 字母对首字母 |
| 拼音大9键 | NINE × PINYIN（同 9 键） | 只差键面字号（`big = true`） |
| 笔画键盘 | STROKE × PINYIN | 五个键 `h/s/p/n/z`（横竖撇捺折），查 `stroke.opid`；底部另有「生僻字」入口 |
| 生僻字键盘 | **不进引擎** | 面板式：**选部首 → 选笔画数 → 挑字**（`keyboard/RarePanel.kt` + `RareIndex.kt`，数据 = `assets/rare_index.tsv`，Unihan 部首+总笔画索引、覆盖扩展区、懒加载 1.1MB） |
| 拼音双拼 | SHUANGPIN × PINYIN | 键面 = 26 键 QWERTY；解码与布局（自然码/微软/小鹤/智能ABC/紫光/拼音加加）在引擎侧，布局在设置页选 |
| 五笔 | WUBI × PINYIN | 键面 = 26 键 QWERTY；码表（86/98）与风格（纯五笔/五笔拼音混输）在设置页「五笔」一节；四码唯一自动上屏默认开 |

- **键面按主流输入法**（2026-10-09 用户裁决「排版太不适应」）：9 键的字母是主显、数字缩小到
  字母下方（此前是反的）。26/14/笔画盘的**底行** `[符/123] [中/英] [空格] [⌫] [↵]`
  （符/面板键在左、中/英其次；此前中/英在最左）。
- **9 键整盘重排**（2026-10-10 用户裁决；`keyboard/NineKeyKeyboard.kt`）：
  `[左列(加宽到 1 键宽)] [3×3 数字格] [右列(与左列同宽)]` +
  底行 `[符][中/英][123][英文][↵]`。**⌫/重输/空格 从底行移入右列**（三格均分格高）；
  底行新增 **[123] 数字键盘**（进数字面板）与 **[英文]**（切英文 26 键，与输入模式面板
  同一条 `selectInputMode` 路——含「切英文=拼音上屏」语义）；↵ 压尾。
  ⚠️ **2026-10-15 订正（用户裁决）**：①3×3 格的**第一格**（原面「，。?!」）换成
  **分词键**（面「分词」、发 `'` 往缓冲插音节边界；缓冲空时是**空操作** —— 宿主
  `onSegment` 拦住，空缓冲的 `'` 在引擎里是引号不是分词符）；②底行 `[中/英]` 那枚键
  在 9 键上**只切简/繁**（键面「繁」/「中」= 点它会切到哪），**不再循环进英文**
  —— 上表与 §4.2 的「中→繁→英」循环描述在 9 键上按这条订正理解（26/14/笔画盘未改）。
- **拼音显示在候选行上方**（2026-10-15 用户裁决「移动端的拼音在候选词上方显示，
  不要在输入框里」）—— **对 2026-10-10「组合串进字段」那条的改判**：拼音改画在键盘里
  候选行**上方**的独立一行（`ime/ComposingBar.kt`），**不再**经
  `InputConnection.setComposingText` 推进输入框（`ComposingText.kt::pushComposingText`
  现在只负责收干净可能残留的组合区）。文本取**显示读数**（9 键 `ni'hao`、其余原字母；
  9 键读数空不落数字 —— 2026-10-11 那条**仍然有效**，只是换了出口）。
  行高 `COMPOSING_BAR_HEIGHT_DP=38`，与候选行同为**常驻槽位**（空时不铺底色），
  并由 `OpiImeService.keyboardHeight` 计入 IME 窗口高度。
  静态门禁：`app/src/test/.../ime/ComposingPlacementTest.kt`（拼音行在候选行之前 +
  不许再往字段写拼音）。
- **待编键的「·」标记**（2026-10-12 晚用户报障「多次点 mno → 点候选确认 → 点 jkl 就无匹，
  点其他键也无响应」的修法）：9/14 键的读数只覆盖**候选命中的前缀**，而 9 键数字串
  本来就有 26% 查不到（真库穷举 11110 串里 2920 个：`44`/`55`/`45`…）—— 以前这些
  **待编的键在字段里一个字都不出现**，再按新键时读数与候选都不变 ⇒ 看着像键盘死了。
  现在按**每个待编键补一个「·」**（`EngineController.pendingKeyMark`：键数 = 缓冲长度 −
  读数里的字母数，`'` 不占键位）：无匹时是 `···`，确认后残留尾巴时是 `mo'mo··`。
  **不给数字**（守 2026-10-11「别变成数字输入」那条）、**不编假拼音**（那会像真读音）。
  提交/清空时组合区干净替换
  （服务侧 `dropComposing` 兜住「提交前那次 refresh 会把组合区落定成普通文本、commitText
  只能追加」的次序坑——真机实测过 `ni'hao你好` 双份残渣）。
- **9 键宽松兜底档**（2026-10-15 用户裁决；报障原话「输入 ni 拼音后不停点 i 所在的按钮，
  然后确认第一个候选词，再次输入其他拼音无反应，候选词显示无匹。……没匹配的用按钮上最前
  的开始匹配字、词」）：**严格档**（主查询 + 补读音 + `scheme_fallback_len` 回退）读不出来
  的那段余键 = **尾巴**；有尾巴时按「键上**最前**的字母」（`key_class('4')[0] = 'g'`）
  起音匹配 —— **先段后键**：整段当一个音（取**最后一个**键的首字母）出字与词，再逐键起音出字。
  尾巴**不分长短**（单键也走 ⇒ 打完一个词再按下下一个音首键时，拼音行显示 `ni'hao'w`
  —— 尾巴从此**看得见**，与上面那条「·」标记各管一段）。判据/封顶在
  `candidates_scheme::tail_guess_hits`，门禁 `crates/engine-core/tests/nine_tail_guess.rs`
  （夹具）与 `crates/opi-tools/tests/m2_integration.rs`（真库）。**14 键本版不启用**
  （键类首字母 == 键自身，会退化成字面前缀）；含 `'`（显式音节主张）或无类键（字母/`0`/`1`）
  的输入整档不启用。⚠️ 连带改判：9 键「无候选时空格不上屏也不清缓冲」的逃生口，现在只剩
  含 `'`/无类键的输入能构造出来（`m2_integration.rs` 那条 `55` 用例已按此改判）。
- **密码类字段自动临时英文**（`isPasswordEditor`）：文本类 PASSWORD / VISIBLE_PASSWORD /
  WEB_PASSWORD ⇒ 自动切英文键盘、字母直出；离开还原、**不落盘**；数字类密码有意不在列。
- **回车决策**（`enterDecision`，纯函数）：`IME_FLAG_NO_ENTER_ACTION` **优先于 action 位**，
  `UNSPECIFIED(0)` 与 `NONE(1)` 都走换行 —— 多行编辑器（常两样 flag 都设）「换行无效」的修复。
- **数字面板点一下立即上屏**：数字直提（引擎侧不映射它字面——标点档只管标点），
  零损失；全角开时在直提点宽化（`EngineController.fullwidthize`）；`,`/`.` 仍走引擎
  （标点表 + 全角两档生效）。
- 工具栏最右「收起键盘」图标 = **向下折角 ⌄**（不是 ✕；✕ 是关闭语义，归关面板/关页）。
  **2026-10-20**：中英文空闲都显示工具条、有候选时仍由候选栏替换；英文 pro 开启后切到中文会自动关闭并落盘。
  候选态下**不再**在候选栏最右放「收起键盘」（2026-10-21 用户裁决：候选占满工具条）；收起入口只在空闲工具条/英文空闲工具条。
  9 键的**符**键 2026-10-09 起进**符号页**（`keyboard/SymbolPage.kt`）：**底部横条** 22 类目
  （`序号.名称`，常用/中文/英文/网络/数字/角标/序号/音标/平假/片假/箭头/特殊/拼音/注音/
  竖标/部首/俄文/希文/拉丁/制表/土音/藏文；数据 = `assets/symbol_categories.tsv`，由
  `scripts/gen_symbol_categories.py` 生成、入库；读取器 `SymbolCategories`；
  2026-10-10 用户订正「分类在最右侧而不是底部」→ 类目条从右竖条移到**底部横条**，可横滑），
  上方网格**左右滑动翻页**、点符号上屏；26/14/笔画盘的符键同路。
  ⚠️ 「常用」类目的内容 2026-10-16 两轮补充（用户裁决「加上数学、物理、化学符号」）：
  含全角标点族 + 数理化符号 —— 判据钉在 `SymbolCategoriesTest` 的两条断言上，
  清单真源 = `scripts/gen_symbol_categories.py` 的「常用」段（不要从本行反向抄）。
  9 键的底行在 中/英 之后另有**重输键**（9 键专属，见上表）。
  ⚠️ **2026-10-15 订正**：本句「重输键在底行中/英之后」与实现不符 —— 重输键自
  2026-10-10 整盘重排起就在**右列**（⌫/重输/空格 三格，见上表与上面那条），
  底行是 `[符][模式键][123][英文][↵]`；模式键键面 2026-10-15 起只在中/繁之间切。
- **9 键的左侧一列**（2026-10-09，主流同款；2026-10-10 宽度 0.72 → **1.0 键宽**）：
  **空闲 = 一列标点**（，。、？！，可点，发送键走引擎中文标点表）；**输入时整列换成
  候选拼音读数**（一个读音一行、**已去重**：你好→`ni'hao`；2026-10-09 用户裁决
  「一列多个、可滑动查看后面候选/下一页」；2026-10-12 用户裁决「左列去重」——
  （同日晚再裁决「排列从短到长」：去重后按显示串长度升序、同长度保持名次序，
  见 linux.md §4.9b；引擎出口 `candidate_readings` 一处改，四端一致）——
  此前逐候选一行，真库 `426` 实测 8 行里 5 行重复）——「拼音是输入才替换到左侧一列」
  （用户原话）。
  数据来自引擎 `candidate_readings` 出口（JNI `candidateReadings`）：与候选**同一次
  排名**、按读音**去重保序**（首现序 = 该读音最好那条候选的名次；去重在引擎出口做，
  内部缓存仍是逐条的），每条按音节用 `'` 连起来；
  列内可上下滑动（与标点列同节奏五格，多出来的滑着看），换一批输入整列回顶部。
  **点一行读音 = 钉住该读音**（2026-10-09 用户裁决「拼音要可选，选择后所有候选词跟着
  拼音变化显示出来，**不要直接确认**」）：候选收窄到**读音恰等**的条目、**不上屏**；
  同一行再点 = 解除；缓冲一变自动解除。钉住的行以强调色显示（`EngineController.
  pinReading` / 引擎 `Engine::set_pinned_reading`，按缓冲配对校验）。
  「恰等」是 2026-10-16 用户裁决（「左侧选择拼音的字了，就不要联想词组了，选择的是字」）：
  此前是「以钉住读音**开头**」⇒ 钉 `me` 时 `美俄/每个`（mei 一族）、钉 `ni'hao` 时
  `你好啊/你好吗` 都会跟在后面；现在只留读那个音的条目（补读音条目按「缺的读音恰是
  钉住那个」放行）。门禁：`crates/engine-data/tests/pin_reading_real.rs`（真库）+
  `crates/engine-core/tests/scheme_readings.rs`（夹具）。
  **只有读得到拼音时才替换**——不输入、回退、确定（提交）之后读数即空，左列回到
  初始化那一列标点（2026-10-09 两条报障的修复；「确定后不恢复标点」的根因是读数
  缓存的次序假设：引擎侧 `scheme_readings` 与缓冲绑存 + Kotlin `refresh()` 读数最后读；
  不回退显示数字串）。候选栏左格在 9 键下**恒空**（2026-10-09 用户二次裁决：「输入无匹配的时候，
  候选出现 logo 图标，logo 图标前就不要显示数字了」—— 数字串在候选区是噪音；
  曾有一版无候选时显示缓冲原文，已被明确撤掉，别加回来）。**2026-10-11 订正**：这条裁
  决当时只落到读数/候选栏两个出口，**其余三个出口漏了** —— 9 键无匹配时
  `pushComposingText` 回退缓冲把数字画进输入框、空格经引擎「无候选提交原文」把数字串
  上屏、读屏播报原样念出键位数字（真机报障「9 键没匹配的时候会变成数字输入」）。现五处
  一致：服务层组合串与播报走 `EngineController.composingText()`（9 键读数空不回退数字），
  引擎 `input_space` 9 键无候选回空串且**不清缓冲**（退格改一键可重来，与符号模式同理由；
  26/14 键「打英文」逃生口不动，钉子 `segment_fallback.rs::nine_key_space_with_no_match_commits_nothing`）。
  26/14/笔画没有左列，
  候选栏左格显示的就是实际缓冲（字母串本来就是拼音）。
- 9/14 键的**匹配在引擎**（`engine-core/src/scheme.rs` 的按键类展开，全量遍历词库逐键比对）——
  键位表在 `keyboard/NineKeyKeyboard.kt` / `FourteenKeyKeyboard.kt`，两边由单测**交叉核对**
  （`NineKeyLayoutTest` / `FourteenKeyLayoutTest` 里各带一份 `scheme.rs` 的快照）。
- 笔画词典 = `assets/stroke.opid`（|rime-stroke|，5.4MB，`gen_stroke_dict.py` 生成、入库）；
  没装/坏了时候选为空（可选增强），键盘照常可打。

**键盘模式（六种）**——落地层各不相同，改之前先看 `ime/KeyboardModes.kt` 的注释：

| 项 | 落地层 | 状态 |
|---|---|---|
| 繁体模式 | **引擎模式**（与「中/繁/英」键同路）；简体模式另有**简体优先**（2026-10-13 裁决：混排时含繁体专属字的候选排后 —— 引擎层，四端一致，见 linux.md §5.3） | ✅ |
| 单手模式 | 布局：键盘区收窄到 76% 并靠左/右 | ✅（纯 Compose） |
| 暗光模式 | 配色：`keyboard/Palette.kt` 的 `DarkPalette`；**覆盖皮肤**（优先级见下） | ✅ |
| 长辈模式 | 配色（`ElderPalette` 高对比）+ 字号整体 +18% | ✅ |
| 悬浮模式 | **IME 窗口**：收窄到屏宽 86%、居中贴底（`OpiImeService.applyKeyboardWindow`） | ⚠️ **无真机可验**；不做拖动 |
| 游戏模式 | **IME 窗口**：高度 ×0.72 + 同样收窄 | ⚠️ 同上 |

⚠️ **悬浮/游戏只在本机静态验证**（编译 + 单元测试），窗口尺寸/重力的真实观感
要真机（分屏、手势条 inset 都可能影响）——别把「CI 绿」读成「真机上好看」。

**皮肤（十二款）**——`keyboard/Palette.kt` 的 `SkinOption`：不改动（默认）/ 浅色 / 深色 /
宣纸 / 朱砂 / 清爽 / 湛蓝 / 柠檬 / 晚霞 / 星光（后五款 2026-10-10 加）/ 紫藤 / 松林（2026-10-13 加）。与桌面设置窗、
fcitx5 配置面**同名**，宣纸/朱砂与后五款的配色取 `scripts/gen_fcitx5_themes.py`
同一套色值。工具条的调色板图标开面板：十二张**预览卡**
（小键盘样张 = 底色 + 键面 + 一枚强调键），点一张换一张、不自动关（连换着比）。

配色三条轴的**优先级**（`ImeScreen.kt` 里那条 `when` 就是它）：
**长辈 > 暗光 > 皮肤** —— 模式是临时覆盖，皮肤是「底」。「深色皮肤」与「暗光模式」
观感相同，两者同开时暗光赢（不冲突）；「不改动」= 不指定皮肤（默认观感）。

输入模式、键盘模式与皮肤都**落盘**（`ImePreferences`，枚举按 name/id 存，未知值回默认）。
**五笔六键**（2026-10-14，M4）同样落盘（`wubi_plan`/`wubi_style`/`wubi_mixed_entry`/
`wubi_candidate_limit`/`wubi_return`/`wubi_auto_commit`）；名字表与「名字 → FFI 整数码」
换算在 `settings/WubiNames.kt`（锁步门禁 `WubiNamesLockstepTest` 现读 `engine_wubi.rs`）。
两条如实记：**方案**（86/98）决定装哪张码表，**下次启动**才装载（`EngineLoader.load`
读偏好、按方案装一张）；**回车行为**引擎只存设置位、四端路由未接线（改动不生效，
设置页文案已写明）。
**未设置过时默认「拼音9键」**（2026-10-11 用户裁决「移动端默认初始 9 键」；已存过选择的
用户读自己的值，不被改写）。
换布局/换窗口模式的生效时机：输入模式立即（引擎 `setScheme` 清缓冲）；键盘模式里的
窗口两项由 `applyKeyboardWindow` 当场重排，不等系统下一次 configure。

### 4.6 设置页

`settings/SettingsScreen.kt`。⚠️ **下表是 2026-10-08 的快照，此后设置页长成了「分节」形态**
（学习 / 模糊音（逐对）/ 拼音纠错 / 简拼 / 拼音方式 / 词库 / 五笔 / 关于），来源是各节的
`*Section.kt`（`DictSection` / `WubiSection` / `AboutSection`）—— 下面只逐条说明本表里的
那几个开关，**不是**设置页的全集。

⚠️ **2026-10-15 装机核验发现的真缺陷（v1.9.2 发布后当即修）**：这一页从建页起外层
`Column` 就没有 `verticalScroll`，内容比视口长时下半截被**裁掉且滑不动**
（1080×2400@420dpi 实测：视口 862dp，`简拼 / 拼音方式 / 词库 / 五笔 / 关于` 全部够不到，
uiautomator 语义树里也没有那些节点 —— 用户报障「看不到关于」的机制就在这）。
修法是外层 `Column` 加 `.verticalScroll(rememberScrollState())`；静态门禁
`settings/SettingsScrollableTest` 钉住它，装机实测（`adb shell input swipe`，慢拖
900–1200ms；快滑在本机模拟器上常被判不成拖动）能一路滑到页尾的「关于」。

| 项 | 行为 |
|---|---|
| 学习（开关） | 关掉就不记用户词频；小欧的表情跟着这个开关走（睡着/醒着） |
| 模糊音（开关） | `z↔zh`、`c↔ch`、`s↔sh`、`n↔l`、`f↔h` 等近音互查（引擎侧 `set_fuzzy`） |
| 简拼（开关） | 声母缩写：`nh → 你好`、`zg → 中国`（引擎侧 `set_jianpin`） |
| 清除用户词库 | 确认对话框 → 清内存里的用户词**并删掉落盘文件**（不删文件的话，输入法下次启动会把它 import 回来，「清除」等于重启后失效） |
| 导出词库 JSON | 复制到剪贴板 |
| 从剪贴板导入 | 与「导出」闭环（复制→粘回来），不经文件管理器 |
| 导入词库 JSON 文件 | 系统文件选择器；MIME 放宽到 `application/json` / `text/plain` / `application/octet-stream`（云盘与 IM 转存的 `.json` 常被判成后两者），有**有界读**上限，超限给可见的失败提示而不是主线程 OOM |

✅ **三个开关已落盘**（2026-10-09 起，关掉了此前记在这里的既有缺口）：存
`SharedPreferences`（`settings/ImePreferences.kt`，文件名 `opi_ime_prefs`），
IME 服务在 `EngineLoader.load` 之后把三个值推给引擎（`prefs.applyEngineSwitches`）、
设置页**先推后读**（设置页可能先于 IME 启动，不推一遍会读到引擎默认值而不是存过的）。
至此四端都已落盘（Linux = fcitx5 addon 配置、Windows = `settings.ini`、桌面设置窗 =
同一个文件）。

**表情中显示自有（2026-10-20 用户裁决，默认关）**：设置页「表情中显示自有」开关，
落盘键 `pet_show_own`（`ImePreferences`，缺省 `false` = 隐藏）。关闭时表情页顶栏
只有 Emoji / 颜文字，不出现「自有」子页；打开后才出现小欧贴纸分类。点选仍走
既有图片通道：PNG → GIF 插入，两种都被拒时剪贴板兜底并上屏 `[小欧·笑]`
文本标签（`PetStickerCommit.kt`），
**不插入任何 PUA 文本**。

`shared/pet/emoji-font/map.tsv` 里的 U+E000–U+E00B 只是 12 个形态的**稳定内部 ID**
（跨端标签/短码表的索引），不随包提供字体，也不会上屏。旧的
「自有表情用字体字符（PUA）」开关与 `opi-emoji.ttf` 已于 2026-10-20 删除。

---

## 5. 验证

### 5.1 词库到底装进去没有 —— 两级证据

词库通路是：**APK 里的 `assets/*.opid` → 解压到 `filesDir/` → `OpiEngine.load(path)`**，编排在 `jni/EngineLoader.kt` 的 `load(context)`（`OpiImeService.onCreateInputView` 与 `SettingsActivity.onCreate` 各调一次）。

⚠️ **luna 每进程只 install 一次**（`EngineLoader.LunaLoadGate`，2026-10-18）：设置页与
IME 同进程共享 Rust 单例，重复 `OpiEngine.load` 会把 IME 正在用的输入方案/模式/开关
重置成默认值 —— 用户报障「打开输入法 App，再到其他 App 输入无反应」的根因就在这
（典型：9/14 键被重置成全键盘，按键不再出候选）。trad/stroke/五笔码表仍在每次调用时
重挂（它们只挂词典，不重置主引擎），所以设置页改码表方案仍会生效。

另有**导入词库层**（2026-10-14 M3）：`filesDir/dicts/*.tsv` 由 `engine/DictStore.kt` 逐个
`importDict`（`OpiImeService` 启动时装载、停用名单装载后重放），设置页「词库」一节管理
（可启停/删除）。**康熙字典随包**（`assets/kangxi.tsv`）：首次启动**种进该目录一次**
（`DictStore.seedBundled`，标记 `.seeded-kangxi`；之后它就是普通导入词库 —— 删过不复活）。
**2026-10-18 起**康熙中默认库还没有的 (字,读音) 也并进了 `luna.opid` / `trad.opid`
（分别 1,279 / 622 条），所以删除或停用 `kangxi.tsv` 不会让这些字重新打不出。

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
adb shell run-as xyz.erik.opi ls -la files/          # 调试包才能 run-as
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
adb shell run-as xyz.erik.opi ls -la --full-time files/
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
adb shell dumpsys package xyz.erik.opi | grep -E 'versionCode|versionName|primaryCpuAbi|lastUpdateTime'
adb shell settings get secure default_input_method          # 当前输入法
adb shell settings get secure enabled_input_methods         # 已启用的输入法列表
```

【实测】本机分别给出 `versionCode=9 …`、`versionName=1.0.16`、`primaryCpuAbi=x86_64`、以及 `xyz.erik.opi/.OpiImeService` —— **`primaryCpuAbi` 是模拟器/真机与 §1.3 那张 ABI 表的交点**，装错 ABI 的包会在这里露出来（或者是运行时 `UnsatisfiedLinkError`）。

### 5.5 出问题时看哪里

| 现象 | 先看 |
|---|---|
| 键盘出来但不出一字、也无报错 | 日志有没有 `engine on builtin fallback dict`（§5.1） |
| 键盘是空白壳 / 根本不出现 | 先看 `OpiImeService` 的 `onCreateInputView` 与 `onEvaluateInputViewShown` 日志。判定按**输入目标生命周期**：`onStartInput(info != null)` 置 active、`onFinishInput` 清掉；**不能按 `inputType`**——抖音直播弹幕框用 `TYPE_NULL` 但仍通过 InputConnection 收字，旧判据会把它误判成“不接受文本”（2026-10-21 修）。外接硬件键盘的模拟器也由 active 这半保住。 |
| 启动即崩、报 `UnsatisfiedLinkError` | `.so` 的 ABI 与设备不符，或 `System.loadLibrary` 失败。注意 `EngineLoader` 的注释明确划了边界：**它只覆盖「词库」层面的失败，`.so` 缺失是另一个失败域，而且发生得更早**（`OpiEngine` 是 object，类初始化里就 loadLibrary） |
| 装了但还是旧词库 | §5.1 第二级 + §3.2 最后那条 `dumpsys` |

---

## 6. 已知限制与未验证项

### 6.1 候选翻页有上限：`fetchLimit` 仍写死 64

**这是端侧限制，不是引擎限制。**

- 现状：`engine/EngineController.kt` 的 `const val fetchLimit = 64`（**以该常量当前值为准** —— 修了它这条限制就没了，本文的 64 只是写作当天从代码里读到的），`refresh()` 里 `api.candidates(fetchLimit)` 一次取这么多条，翻页（每页 `pageSize`）纯客户端做，**引擎侧没有 offset**。
- 引擎侧早就没有这个上限了：`FETCH_LIMIT` 与 `Engine::select` 内部那道截断都已拆掉，三处同值（`engine-core/src/router.rs` · `fcitx5-opi/src/candidate.rs` · `tsf-opi/src/logic.rs`，互相之间**有等值断言**）。【实测】后两处今天都是 `pub const FETCH_LIMIT: usize = usize::MAX;`，紧邻一行就是与 `engine_core::router::FETCH_LIMIT` 的 `assert!`。
- **订正（2026-10-11）**：「引擎侧没有上限」只对**精确层**成立 —— 大词库整合后，**前缀扩展层**与 **9/14 键按键类层**各有静态词频 top-N 截断（`dictionary.rs` 的 `PREFIX_MATCH_CAP=9216` / `SCHEME_MATCH_CAP=2048`，缘由与实测见 [`docs/performance.md`](../performance.md)）。Android 一次取 64 条 ≪ 两个上限 ⇒ 本节的实际结论（翻不到第 64 条之后）不受影响。
- 也就是说：**长前缀拼音打出的候选超过 64 条时，Android 端翻不到第 64 条之后**。macOS 端是同一个病：【实测】`macos/InputController.swift` 调 `engine.candidates(limit: 64)`，而默认值也是 64（`macos/OpiEngine.swift` 的 `candidates(limit: Int = 64)`）。
- 【未验证】「长前缀能不能翻到末页」这件事**在 Android 真机/模拟器上没跑过** —— 这一条是静态取证（读 `EngineController` 的常量 + 引擎侧门禁），不是实测。

### 6.2 本机验证不了、且确实没验过的

| # | 事项 | 依据 |
|---|---|---|
| 1 | ~~`assembleDebug` 过不去：缺 Dart SDK~~ **已解决（2026-10-11）**：Dart 已在，debug/release 均实测构建 | §2.4 订正 |
| 2 | ~~`assembleRelease` 从未真正构建过~~ **已实测（2026-10-11）**，且为 dist 分发口径 | §2.3 订正 |
| 3 | 符号面板搜索态的 96dp 叠盘在**高 density 机型**上的实际观感 | `ImeScreen.kt` 该处注释自称「本机无设备，此改动未实测」 |
| 4 | 横屏下键区占屏高（注释称 57%→68%，未在真横屏机上验） | `OpiImeService.keyboardHeight` 的注释 |
| 5 | 真机（`arm64-v8a` / `armeabi-v7a`）上的任何行为 | §1.2 本机只有 x86_64 模拟器 |
| 6 | 候选翻页上限（§6.1）的真实影响面 | §6.1 |
| 7 | 覆盖安装后的自动重载全链路（kill 后系统是否/何时重绑 IME；OEM 是否投递 `MY_PACKAGE_REPLACED`） | §3.4，只有 JVM 配置面门禁 |

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
adb shell ime enable xyz.erik.opi/.OpiImeService
adb shell ime set    xyz.erik.opi/.OpiImeService

# 验证
adb logcat -d -s EngineLoader:V OpiImeService:V                 # 词库装载字节数
unzip -v android/app/build/outputs/apk/debug/app-debug.apk | grep opid   # 应为 Stored / 0%
adb shell run-as xyz.erik.opi ls -la --full-time files/         # 幂等：mtime 不该变
adb shell dumpsys package xyz.erik.opi | grep versionCode        # 机上版本
```
