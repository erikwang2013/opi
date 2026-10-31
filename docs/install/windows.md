<!-- SPDX-FileCopyrightText: 2026 erik.xyz -->
<!-- SPDX-License-Identifier: MIT -->

# Windows 端（TSF）编译 / 打包 / 使用

> ### v1.26.0 功能注记（2026-10-26）
>
> - **学习加权/撤销与英文频次**是引擎语义，TSF 直接继承（空格 +1、显式选词 +2、同输入改选撤销）。
> - **用户词仍是内存态**：本版没有把 fcitx5/macOS 的 `opi_user_words.json` 接到 TSF；进程重启后学习丢失。
> - **外部词库**：`%LOCALAPPDATA%\opi\dicts\*.tsv` 走 `load_extra_dict`，无 8 MiB / 20 万条限制；完整 34–39 MiB 词库可直接放。
> - **未接**：括号自动配对只在 Android/iOS/鸿蒙；Windows 桌面端没有光标回移。9/14 键按键类池 2048，26 键全量池 9216。

> ### 先读这一段
>
> **Windows 已由用户在真机验收（安装 / 打字 / 候选窗 / 中英文切换 / 状态栏；覆盖安装
> 免注销，已开应用重开生效）。本文仍是操作步骤与排障手册，不是一份覆盖所有宿主/签名
> 组合的配方。**（iOS / macOS / 鸿蒙已由 CI 或本机工具链真编译并产出 unsigned 产物；
> 它们仍未真机验收。）
>
> **【订正 2026-10-21】** 安装/卸载已在真 Windows 10 上实测（1.20.2 EXE 安装器：
> 正常卸载 / DLL 被映射 + 候选窗运行 / DLL 缺失 / `/DELETEUSERDATA`，89/89 项通过；
> 日志为 `Removed all? Yes`、`Need to restart Windows? No`，Explorer PID 未变）。
> **打字/候选窗/中英文切换/状态栏也已在 Notepad 真机验证**（§5.3）。**仍未验证**
> 的是**交互式**安装向导、语言栏图标、非 Notepad 宿主与候选点选的点击回程手感；
> 安装仍用 `/VERYSILENT`。同日升级路径真机验证：`luna.opid` 被旧 TSF DLL 映射时
> 在 ssInstall 改名移出，日志 `Need to restart Windows? No`；旧版留下的
> `PendingFileRenameOperations` 由 `InitializeSetup` 清理。「没人装过、没人跑过」
> 是本文初稿的表述，不再适用于安装/卸载/打字这一层。
>
> **【订正 2026-10-19】** 用户已在真 Windows 上装过 Release 的 v1.15.1 MSI：应用进程能起，
> **但输入法键盘不出现**。根因用该 MSI 的表级证据钉死 —— 那是 **jpackage 原始产物**，
> `CustomAction`/`InstallExecuteSequence` 里没有任何注册动作，安装时**从未执行
> `regsvr32`**（TSF 的 COM/类别/语言 profile 全未注册）。本轮已把后处理接入 CI
> （§2.5 的 `msi-postprocess` job），并把安装目录改为 `C:\Program Files\opi`。
> **后处理后的包仍然没人装过** —— 下面所有【未验证】条目不因此消失。
> 2026-10-09 起多了**「能被构建出来」**这一档：DLL 与 .msi 会在 GitHub 的 windows runner 上
> 真编出来（§2.5）。**「装上」「跑起来」仍然一次都没有** —— 别把 §2.5 读成验收通过。
>
> **【订正 2026-10-20】** Windows 的**包名 / 启动器 / 安装器一律叫 `opi`**（不再叫
> `opi-candidates`），默认安装目录 `C:\Program Files\opi`。用户入口改为 CI 打出的
> **EXE 安装器** `opi-<版本>-windows-x64-setup.exe`（Inno Setup，job `desktop-exe`，§2.5）：
> TSF 注册在文件落盘后执行 64 位 `regsvr32 /s`，**只记录退出码，失败不回滚安装**。
> 后处理 MSI 降为**内部证据**（artifact 现名 `opi-msi-internal-v<版本>`），它的注册
> 动作仍是 Type 1、失败会回滚整包 —— 这正是 Server Core 上「根本装不上」的旧形状。
> EXE 的容器内静默安装/卸载冒烟已在 2026-10-20 的 v1.17.0 run 跑绿
> （`windows-exe-install-test`：安装/卸载退出码均为 0，`regsvr32` 退出码 0），
> 但**仍无真机语言栏/打字验证**。
>
> 每一条都带证据档位：
>
> | 标记 | 含义 |
> |---|---|
> | **【实测】** | 在 Linux 开发机上真跑过（主体是 2026-09-28 那一批；另跑过的在文中标注日期），命令与输出在文内 |
> | **【CI 实测】** | 2026-10-09 在 GitHub 的 `windows-latest` runner 上真跑过（`.github/workflows/windows.yml`），命令与输出在 §2.5 |
> | **【推断】** | 从源码读出来的结论，给出文件 + 符号；没有运行过 |
> | **【未验证】** | 本机没有 Windows，**没有任何人跑过这一步** |
>
> 本文里**【未验证】**的条目占多数 —— 这是现状，不是本文没写完。判定依据是
> `README.md` 的里程碑 M6c：「COM 服务端按目标平台门控，**且仓库里没有打包步骤**
> （词库开箱仍是内置回退那几十条词），待在 Windows 上验收」。
>
> **【订正 2026-10-20】** 引文里「仓库里没有打包步骤／词库开箱仍是内置回退」已不成立：
> `desktop/build.gradle.kts` 的 JPackage `files` 集合现在把入库副本
> `android/app/src/main/assets/luna.opid` 放进 app image 的 `app\luna.opid`（DLL 同目录），
> EXE 与内部 MSI 都携带，CI 在 app image 布局与 EXE 安装冒烟两处断言（§2.4）。
> `README.md` 的 M6c 一行尚未同步（本轮不改根 README，以 §2.4 为准）。
> **「包里有」仍不等于「真机上 TSF 已加载」**（装载日志走 `eprintln!`，GUI 宿主看不见）。
>
> **别把本文当"照此即可"** —— 本项目的既有教训正是「写完了、读起来像完成、实测发现
> 从未被编译器看过」（fcitx5 的 C++ 7 处 API 误写、TSF 曾有的"一个字都插不进去"）。

---

## 1. 编译前提

### 1.1 目标机器上需要什么

| 项 | 值 | 依据 |
|---|---|---|
| 操作系统 | Windows 10 1607+ / 11 x64 | 目标三元组只有 `x86_64-pc-windows-msvc`（下面 1.2 有实测）；**Windows on ARM 需要另加 `aarch64-pc-windows-msvc`，本仓库没有为它做过任何事** |
| C++ 构建工具 | Visual Studio 2017+ 或 Build Tools，勾选「使用 C++ 的桌面开发」 | 提供 `link.exe` 与 MSVC 导入库。`crates/tsf-opi/Cargo.toml` 声明的 `windows` crate 依赖在链接期需要导入库 |
| Rust | `rustup target add x86_64-pc-windows-msvc` | 目标不是默认安装的 |
| 词库文件 | `luna.opid` + `trad.opid`（繁体，可选），**安装包已自带**（`app\`，见 §2.4）；手工部署时放 DLL 同目录或 `%LOCALAPPDATA%\opi\` 亦可（§2.2） | `crates/tsf-opi/src/dict_path.rs` |
| 候选窗运行时 | JVM（只有要显示候选窗时才需要） | `desktop/` 是 Compose Desktop / JVM 工程 |

> **Windows 10 1607 以下、XP / Vista / Win7 / Win8 / 8.1 不支持（2026-10-31 复核）**：当前
> `tsf_opi.dll` 导入 `api-ms-win-core-synch-l1-2-0`（Win8+）与 `bcryptprimitives`
> （Rust 1.88 std 的 Win10 路径）；候选窗是 JDK 21/Temurin/jpackage + Compose
> Desktop 1.11，官方目标 Win10/11。要在 Win7/8 上运行须另建 legacy 轨道
> （Rust ≤1.77 + windows_7/8 + 老 TSF 注册 + 原生候选窗 + 独立安装包），不在本仓库范围。

### 1.2 `cargo check` 与 `cargo build` 的区别 —— 这条最容易读错

`crates/tsf-opi/src/` 下**四个模块整个在 `#[cfg(target_os = "windows")]` 里**
（`tsf.rs` · `com_server.rs` · `state.rs` · `candidate_io.rs`，见 `src/lib.rs` 的模块声明）。
Linux 主机上它们**一行都不参与构建**。本机能碰到它们的**唯一**动作是给 Windows 目标做类型检查：

```bash
cargo check -p tsf_opi --target x86_64-pc-windows-msvc
```

**【实测】2026-09-28 本机（Linux，rustc 1.97.1）执行结果：**

```
    Checking engine-core v1.3.1 (/home/wwwroot/bag/opi/crates/engine-core)
    Checking engine-data v1.3.1 (/home/wwwroot/bag/opi/crates/engine-data)
    Checking tsf_opi v1.3.1 (/home/wwwroot/bag/opi/crates/tsf-opi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 35s
EXIT=0
```

**这一条证明什么、不证明什么：**

| | `cargo check --target ...` | `cargo build --target ...` |
|---|---|---|
| 词法/语法/宏展开 | ✅ | ✅ |
| 类型检查、借用检查 | ✅ | ✅ |
| `#[implement]` 宏展开出的 vtable 形状 | ✅ | ✅ |
| `const` 断言求值（`vk.rs` 里对 windows crate 常量的那两条） | ✅ | ✅ |
| 生成机器码（`.o`） | ❌ | ✅ |
| **链接**（`link.exe` + MSVC 导入库） | ❌ | ✅ |
| **产出 `tsf_opi.dll`** | ❌ | ✅ |
| 导出表里真有 `DllGetClassObject` / `DllRegisterServer` | ❌ | ✅（链接期才定） |
| `regsvr32` 能加载、注册表能写成功 | ❌ | ❌ |
| 能切到这个输入法、能打字 | ❌ | ❌ |

**边界在"链接"这一步，而且本机实测过**（不是推的）：

```bash
cargo build -p tsf_opi --target x86_64-pc-windows-msvc
```

**【实测】2026-09-28 本机执行结果：**

```
   Compiling tsf_opi v1.3.1 (/home/wwwroot/bag/opi/crates/tsf-opi)
error: linker `link.exe` not found
  |
  = note: No such file or directory (os error 2)

note: the msvc targets depend on the msvc linker but `link.exe` was not found
note: please ensure that Visual Studio 2017 or later, or Build Tools for Visual Studio
      were installed with the Visual C++ option
error: could not compile `tsf_opi` (lib) due to 1 previous error
EXIT=101
```

同一次运行里 `tsf_opi` 的 `.rcgu.o` 目标文件**确实生成了**（codegen 通过），
但产物目录里**零个 `.dll` / `.lib`**。所以准确的说法是：

> **本机能把 TSF 代码一路编到目标文件，然后卡在链接。它从未被链接过，因此从未产出过 DLL。**

顺带两条配套事实，避免误读成"装个链接器就行了"（都是 **【实测】**）：

```bash
ls .cargo/config.toml          # 不存在 —— 仓库里没有链接器覆写
find ~/.rustup/toolchains/stable-*/lib/rustlib -name 'rust-lld*'   # 有：rust-lld 随工具链装着
command -v lld-link lld ld.lld # 全无
find / -maxdepth 6 -iname 'kernel32.lib'   # 零命中 —— MSVC 导入库一个都没有
```

⇒ 卡住链接的**不只是**"缺 `link.exe` 这个名字"：即便把 `rust-lld` 配成 `lld-link` 的形态，
**MSVC 的导入库（`kernel32.lib` 等）在这台机器上一个都没有**。本文**不**把这条路写成可行步骤
（**没有验证过**，也超出了本文范围）。

CI 里针对 Windows 目标的**编译类步骤只有这一条**，也是 `cargo check`：

```bash
grep -n 'tsf_opi' .github/workflows/ci.yml
```

**【实测】**命中**两行**（步骤名一行 + `run:` 一行，都是
`cargo check -p tsf_opi --target x86_64-pc-windows-msvc`）。
同一个 job 的 `runs-on` 是 `ubuntu-latest`（**【实测】**全文四个 `runs-on` 全是它），
⇒ **不链接、无产物**。另有一步 `dtolnay/rust-toolchain` 的 `targets:` 把这个目标预装进 runner
（同处还列着四个 Apple 目标），那是**安装**不是**构建**。

> **【订正 2026-10-09 / 措辞订正 2026-10-19】** 上面这两段说的是 `ci.yml` 的 Windows
> 相关部分 —— 它至今仍然成立（那条 `cargo check` 仍是它唯一碰 Windows 目标的编译步骤，
> 仍**只到类型检查**；`ci.yml` 后来新增的 `packaging-scripts` job 只跑 Linux 安装脚本）。
> 但仓库现在多了一份**只在 windows runner 上跑**的 `.github/workflows/windows.yml`：
> 它做的正是上表里打 ❌ 的那几格 —— `cargo build --release`（**真链接**）→ `tsf_opi.dll` →
> `dumpbin /exports` 读导出表。命令与实测输出见 §2.5。

> 另一个容易混的对照：Linux 主机上 `cargo test -p tsf_opi` 是**能跑的**，
> 但它只跑平台中立的那几个模块（`logic` / `vk` / `dll` / `dict_path`）。
> **【实测】2026-09-28 本机：`cargo test -p tsf_opi --no-fail-fast` 全绿，
> 全部落在平台中立模块。** 那四个 `cfg(windows)` 模块里的代码，
> 除了上面那条 `cargo check`，**没有任何测试碰过它们**。

### 1.3 在 Windows 上编译

```bat
cargo build --release -p tsf_opi --target x86_64-pc-windows-msvc
```

**【推断】**产物是 `target\x86_64-pc-windows-msvc\release\tsf_opi.dll`
（`crates/tsf-opi/Cargo.toml` 的 `[lib] crate-type = ["cdylib", "lib"]`，
包名 `tsf_opi` 未覆写 lib name ⇒ Windows 上 cdylib 就是 `tsf_opi.dll`）。

> ⚠️ `crates/tsf-opi/src/com_server.rs` 的文档注释里写的是 `regsvr32 opi_tsf.dll` ——
> **那是注释里的示意名，不是产物名**。以 `Cargo.toml` 的包名为准。

**【未验证】**：本机无法执行这一步。请以那份构建的实际输出文件名为准，别照抄上面这行。

---

## 2. 打包

### 2.1 产物构成

| 文件 | 来源 | 说明 |
|---|---|---|
| `tsf_opi.dll` | `cargo build --release -p tsf_opi --target x86_64-pc-windows-msvc` | TSF COM 服务器（进程内）。**必须注册才能用**，见 §3 |
| `luna.opid` | `android/app/src/main/assets/luna.opid`（构建期经 `desktop/build.gradle.kts` 放进 app image） | 词库；安装后位于 `app\luna.opid`（与 DLL 同目录），缺失时才退回内置词库 |
| 候选窗（JVM 应用） | `desktop/` | **独立进程**，见 §2.3 |
| （语言栏图标） | `crates/tsf-opi/assets/opi-langbar.ico` | **编译期嵌入 DLL**（`include_bytes!` + 运行期 `CreateIconFromResourceEx`）—— 打包时**不用**带上它；换图标要重编 DLL |

### 2.2 词库通路

`crates/tsf-opi/src/dict_path.rs` 定义了**四级查找顺序**（`dict_path::candidates`）：

| 优先级 | 位置 | 来源标记 |
|---|---|---|
| 1 | `OPI_DICT_PATH` 环境变量的值 —— **文件全路径，不是目录** | `Origin::Env` |
| 2 | **本 DLL 所在目录** 下的 `luna.opid` | `Origin::DllDir` |
| 3 | `%LOCALAPPDATA%\opi\luna.opid`（目录名小写 `opi`） | `Origin::LocalAppData` |
| 4 | 内置回退词库 | `DictLoad::source == None` |

行为细节（都来自 `dict_path::candidates` / `load_dict`）：

- **环境变量为空串等同未设置**（`set OPI_DICT_PATH=` 是常见清空操作，不会被当成"当前目录下的空文件名"）。
- 候选**不存在** → 静默试下一个；候选**存在但损坏/读不出** → 记一条说明再试下一个（**不静默、也不因此让输入法没有词库**）。
- 四个候选全败 → 内置回退词库，**服务照常创建**（源码明写：服务创建不出来会让用户看到"输入法整个不在"，比词库小更糟）。
- 第 2 级取的是**本 DLL 的目录**，不是宿主 EXE 的路径、也不是当前工作目录（`com_server::own_module_path` 用 `GetModuleHandleExW` 的 `FROM_ADDRESS` 反查自己的模块）。

**桌面窗的「词库」页与导入词库（2026-10-14 / 2026-10-16）**：Windows 的 Compose 设置窗
（与 TSF DLL 同一个 MSI 分发）有「词库」页，管 `%LOCALAPPDATA%\opi\dicts\*.tsv`
（导入 / 启停 / 删除）——与上面第 3 级同一条目录约定；**康熙字典随包**，开窗时
首启种子一次（之后照常启停，删过不复活）。生效时机（2026-10-20 起）：保存/导入/删除后
经控制面 `reload` 让 TSF **立即重读** `settings.ini` 与词库目录（与 `Activate` 同一条
`apply_settings`）；输入法没在跑时才退化为「下次激活」生效。

**词库文件从哪来？**

`luna.opid` **不在 git 里**（`data/generated/.gitignore` 忽略它，只放行 `fallback.opid` 与 `trad.opid`）。
入库的那一份在：

```
android/app/src/main/assets/luna.opid
```

**【实测】2026-09-28 本机：** `android/app/src/main/assets/luna.opid` 与本机
`data/generated/luna.opid` 的 `sha256sum` **相同**（复核命令：
`sha256sum android/app/src/main/assets/luna.opid data/generated/luna.opid`）。
要重编，用 `scripts/gen_luna_dict.py`（它需要上游的 `luna_pinyin.dict.yaml`，
**那个文件不在本仓库里**，脚本头注释记着获取方式与当前 master 的 sha256）。

### 2.3 候选窗（`desktop/`）

候选窗是**另一个进程**：Compose Desktop / JVM 应用，经命名管道与插件通信
（协议镜像两份：`crates/tsf-opi/src/candidate_io.rs` 与
`desktop/src/main/kotlin/xyz/erik/opi/candidate/Main.kt` 的模块头注释）。
2026-10-25 起是**两条管道**：新版 DLL 走 `\\.\pipe\opi-v2`，升级前已打开的
应用里的旧 DLL 仍走 `\\.\pipe\opi`；候选窗进程两条都服务。

- **不开它也能打字**：`CandidateClient::connect` 连不上时**单次尝试、本次发送放弃、不睡眠**
  （2026-10-18 性能审计：旧实现在按键线程上 4×50ms 退避，候选窗未启动时每键白卡 200ms；
  现在下一条 show 自然再试，窗口状态每次全量重发），降级为 no-op（**【推断】**，
  见 `candidate_io.rs` 的 `connect` / `send_json`）。用户看到的是"没有候选窗"，
  不是"输入法坏了"。`WriteFile` 仍无超时（候选窗假死时理论上可阻塞按键线程，
  异步写线程是后续项）。
- **单实例管道与「其他应用不生效」（2026-10-20；2026-10-21 真机订正）**：管道只有
  1 个实例（`PipeServer.kt` 的 `MAX_INSTANCES = 1`），而每个加载 TSF DLL 的宿主进程各
  有一个 `CandidateClient`；客户端只在 `show` 时惰性连接、且此前从不主动断开。于是
  **第一个输入过的应用把实例占到进程结束**，其余应用的 `CreateFileW` 一直
  `ERROR_PIPE_BUSY`，`show` 被静默丢弃 —— 这是「输入法只在桌面/单个应用可用」的
  另一条源码级根因。
  **真机验证：`ITfTextInputProcessor::Deactivate` 不随应用切换触发**（只在输入法被
  取消激活时触发），只靠 `Deactivate` 断开不够 —— 后台宿主会一直占着管道，状态栏读到
  它的 `im=opi`（切走不消失），前台应用连不上（中文模式无候选）。修复分两层：
  `Deactivate` 调 `TsfSink::disconnect`；`Activate` 另注册 `ITfThreadFocusSink`
  （`thread_focus.rs`），焦点进入线程 → 主动连接，焦点离开 → 断开。窗口侧 `readLoop`
  读到 0 字节后在 `finally` 隐藏窗口并重建实例。
  **2026-10-21 真机（Notepad×2 + Win+Space，1.20.2 新 DLL）已验证**：A 打字候选窗出现
  → 切到 B 打字同样出现 → 切回 A 仍正常；Win+Space 切到别的输入法后 `state` 变空、
  状态栏约 3–4s 内隐藏（3 拍轮询宽限），切回 opi 后重新出现；物理键盘
  `Ctrl+'` / `Shift+Space` 翻转的 `lang=`/`fw=` 与状态栏图标一致。
- **热升级隔离（2026-10-25）**：覆盖安装后，已打开的应用仍映射旧版
  `tsf_opi.dll`（Windows 不能热替换进程内映像）。旧版 DLL 只认
  `\\.\pipe\opi`，若继续与新版共用一个单实例管道，旧宿主会把管道占住 ——
  新前台应用连不上（中文无候选），状态栏读到旧宿主的 `im=opi` 后切走也不消失。
  修复：新版 DLL 改连 `\\.\pipe\opi-v2`；候选窗进程同时监听 v2（新版）与
  `opi`（旧版），v2 的消息优先，旧客户端断开不会藏掉 v2 的窗口。控制面同步
  升为 `\\.\pipe\opi-control-v2`，状态栏只认新版客户端。**旧管道名永不改**
  （这是对旧 DLL 的兼容面）；协议再有不兼容变更时 v2 升 v3。
  另加候选窗单实例锁（`CandidateLock.kt`）：重复启动的 `opi.exe` 直接退出，
  不再留下多个常驻进程。
- **登录自启 ≠ 常驻渲染（2026-10-21 订正）**：候选窗**始终创建一个隐藏的 AWT/Skiko
  窗口**。曾试过「首次 `show` 之前不建窗」，但 Compose 1.11 的 `application` 在没有任何
  Window 时会在 `Recomposer.close/join` 后直接返回 —— 候选进程启动即退出，两条命名管道
  都不存在。窗口隐藏时不绘制：Skiko 0.144.6 的 `Direct3DRedrawer.frameDispatcher` 在
  `SkiaLayer.isShowing()` 为 false 时直接跳过 `update`/`draw`，`FrameDispatcher` 由
  `needRender` 按需调度（不是死循环）。**【未验证】**：Skiko 那半是 Linux 上反编译
  0.144.6 jar 的静态证据，Windows 真机的 CPU 占用没有测量；高 CPU 的另一条已定位路径是
  旧安装脚本杀 Explorer 造成的 shell 重启循环（已移除，见 §2.5）。
- **候选窗形态（2026-10-21 用户裁决「长条的，和搜狗输入法一样」；2026-10-22 订正为
  两行）**：高固定 76dp，第一行 = 已输入拼音 / 模式，第二行 = 1..8 候选 / `‹ 页码 ›`
  （用户裁决「候选词中的拼音要在候选词上方」）。宽度按两行里更宽的一行实测
  （`rememberCandidateBarSize`）在屏幕内伸缩；放不下时候选区**横向滚动**（整词不省略、
  不缩字号），翻页控件钉在右端。位置：TSF 每次 composition 变化发 `position`（`caret.rs`
  取选区末尾的屏幕矩形，按输入窗口 DPI 折算成逻辑像素）→ 窗口贴着 caret 下方，接近屏幕
  边缘时收回虚拟桌面内；没收到过位置时按宽度水平居中。候选窗不可聚焦，点击不会夺走宿主
  焦点（页面点击回程仍受 §4.3 的"下一次按键才插入"约束）。**候选窗跟着皮肤走**
  （2026-10-22 用户报障「点击状态栏 logo 切换背景，候选词框的背景没变化」）：候选进程
  与状态栏一样每秒重读 `settings.ini` 的 `Background=` / `Skin=`，动态主题按其
  `defaultSkin()` 配色，用同一份 `SkinPalette.skinColors`；动态主题下候选窗
  **整幅铺该主题插画**，再压 52% 主题底色保证候选文字可读（2026-10-31 用户裁决
  「桌面端只换状态栏，候选词也要换」）。
- **状态栏按钮可操作（2026-10-21）**：按钮有按下/悬停反馈；1s 状态轮询在**读取失败时保留
  上一次状态**（连续 3 拍读不到才回到中性），不会因一次管道抖动把整条隐藏（Windows 上
  `im` 未知 = 隐藏，用户体感会是"按钮突然点不动"）。**点击按钮不让条消失**：动作后的
  状态补读失败不覆盖 `im`；打开设置/符号/软键盘时条强制保持可见。**设置/符号/软键盘
  在 UI 线程直接开窗**（不排 IO 队列、不等状态补读），实测设置窗 <1s 出现。
- **状态栏出现时机（2026-10-21）**：TSF 客户端在 `Activate` 时**主动**连候选窗（不再等
  第一次 `show`）。状态栏的 1s 轮询随即能读到 `im=opi`，因此切到「小欧输入法」后状态栏
  **立即出现，不必先打一个字**。`Deactivate` 先置 inactive 再断开，避免读线程把单实例
  管道又抢回去。**注意**：安装/升级后，已在运行的宿主仍持有旧 DLL 映像；EXE 交互式
  升级只在探测到 Explorer 映射旧 DLL 时重启原用户 Explorer（2026-10-23 白屏回归：不能对未映射旧 DLL 的健康 shell 无条件 taskkill）。
  2026-10-25 起其它宿主不必强制重开：旧 DLL 仍可连兼容管道出候选，重开一次即切到 v2；
  状态栏只显示 v2 状态（旧应用前台时条暂时隐藏，重开后恢复）。
- **状态切换即时（2026-10-25 用户要求 Windows 响应 <10ms）**：候选窗进程在主
  候选管道客户端接入/断开时 `SetEvent` 一个命名事件
  （`Local\opi-state-changed-v2`），状态栏线程立即醒来补读，不再等 1s 轮询；
  无客户端时控制面回 `im=` 空状态，状态栏不再走 3 拍宽限，切到别的输入法立刻
  隐藏。Shift 切中英、全半角、模式热键还会发一条 `{"type":"state_changed"}`
  （窗口内容不变，只唤醒状态栏）。控制面的读请求轮询从 10ms 降到 1ms；
  桌面进程启动时 `timeBeginPeriod(1)` 把候选管道 `Thread.sleep(2)` 的实际粒度
  从 ~15.6ms 降到 ~1–2ms。
  **边界**：10ms 是状态/控制路径的目标；候选窗/状态栏最终由 Compose 绘制，
  受显示器帧率约束（60Hz ≈ 16ms/帧），这一层需真机实测。
- 打包格式**当前只声明了 Deb**：`desktop/build.gradle.kts` 的
  `nativeDistributions { targetFormats(TargetFormat.Deb) }`，同一处的注释写着
  「Windows .msi 为验收阶段（Windows 主机上构建）」。
  ⇒ **要在 Windows 上出 `.msi` / `.exe`，得先改这个 `targetFormats`，且必须在 Windows 主机上构建**
  （jpackage / Compose Desktop 不能从 Linux 交叉打包 Windows 安装包）。**【未验证】**

  > **【订正 2026-10-09】** 「只声明了 Deb」已不成立：`targetFormats` 现在列 `Deb` + `Msi`。
  > 加这一项的理由是**机制性**的、不是偏好：CMP 1.11.1 只为 `targetFormats` 里**列出的**格式
  > 注册 `package<格式>` 任务（反编译核实 + 实测：只列 Deb 时 `packageMsi` 任务**不存在**，
  > 报 `Task 'packageMsi' not found`，不是「任务在、只是跨平台打不出来」）。
  > 于是下一条「必须在 Windows 主机上构建」也**已经落地**：`.github/workflows/windows.yml`
  > 在 windows runner 上真打 `.msi`（§2.5）。Linux 侧 deb 路径未受影响
  > （**【实测】**2026-10-09 本机 `./gradlew packageDeb --dry-run --offline` 配置期照常 ——
  > 是 dry-run，没有真打 deb；真打 deb 的是 `packages.yml` 里那条既有 job）。
- 不改打包、只想跑起来：`desktop/` 是自带 wrapper 的 Gradle 工程，
  Windows 上用 `gradlew.bat`（`desktop/gradlew.bat` 存在，**【实测】**文件在）。
  `build.gradle.kts` 里设了 `mainClass = "xyz.erik.opi.candidate.MainKt"`，
  故 `.\gradlew.bat run` 应当能直接起窗。**【未验证】**（本机只有 Linux，没有在 Windows 上跑过）。
  2026-10-09 起还多一条路：直接装 §2.5 的 `.msi` —— 但**没人装过**，两条路都还是【未验证】。

### 2.4 现状：词库随包（2026-10-20 起）

> **【订正 2026-10-20】** 本节 2026-10-09 的旧结论「没有打包步骤／开箱仍是内置回退」
> 已不成立。现在的构建期落点由 `desktop/build.gradle.kts` 的 JPackage `files` 集合
> （`OPI_TSF_DLL` 分支）提供：入库副本
> `android/app/src/main/assets/luna.opid` 被放进 app image 的 `app\`，与
> `tsf_opi.dll` 同目录 —— 正是 `dict_path` 的第二候选，开箱命中。
> EXE 与内部 MSI 共用这一 app image，所以两者都携带；同一批还带三份许可文本
> （`LICENSE`、`data/raw/LICENSES.md`、`data/raw/GPL-3.0.txt`，与 `scripts/nfpm.yaml`
> 的分发义务一致）。
>
> CI 在 §2.5 的两处断言它真的在：app image 布局检查 `app\luna.opid` 存在且 SHA-256
> 与入库副本一致；EXE 静默安装后断言 `C:\opi-test\app\luna.opid` 存在。
> **「包里有」仍不等于「真机上 TSF 已加载」**：装载日志走 `eprintln!`，GUI 宿主
> 没有 stderr，这一步只能在真机上看语言栏/打字结果。
>
> （历史原文，2026-10-09，已失效：）
> > 仓库里没有任何打包步骤会把 `luna.opid` 和 `tsf_opi.dll` 放到一起。词库通路是通的
> > （`dict_path.rs` 的四个候选），但没有人往那四个位置送货；装到 Windows 上开箱
> > 仍然是内置回退词库，除非用户自己设 `OPI_DICT_PATH`。
>
> 旧依据是当时 `dict_path.rs` 模块文档、`crates/tsf-opi/` 无 `build.rs`、
> `README.md` 的 M6c 三处；前两处已随本次改动更新，`README.md` 的 M6c 一行尚未同步
> （本轮不改根 README，以本节与 §2.5 为准）。

⇒ 本文 §3、§4 里涉及 `luna.opid` 的步骤现在都是**可选的手工动作**（换词库、排障），
正常安装不需要用户送货。`trad.opid` 自 2026-10-28 起**也随包**（`app\trad.opid`），
由 `dict_path::load_trad` 装载；缺/坏时繁体模式回退主库（见 §6.2）。

### 2.5 自动打包（GitHub Actions，2026-10-09 起）

`.github/workflows/windows.yml` —— 构建 job 在 windows runner 上跑，后处理 job
（`msi-postprocess`，2026-10-19 起）在 `ubuntu-latest` 上用 msitools 跑，
容器安装冒烟 job（`windows-install-test`）在 `windows-latest` 上跑。
**2026-10-20 起**用户入口改由 `desktop-exe`（windows-latest，Inno Setup）构建，
另有 `windows-exe-install-test` 在 Server Core 容器里静默安装/卸载那个 EXE。
**订正 2026-10-19**：不再写「`ci.yml` 一个字没动」—— `ci.yml` 已有独立的
`packaging-scripts` job（2026-10 起，跑 Linux 安装脚本回归），但它不碰 Windows 构建；
`packages.yml` 未动。它覆盖 §1.2 那张表里本机打 ❌ 的几格：
**DLL 真链接（静态 CRT）、导出/导入表检视、`.msi` 产出、表级后处理（注册动作/安装目录）、
容器内 `msiexec` 冒烟日志、EXE 产出、EXE 静默安装/卸载冒烟**。后处理 job 只做表级注入；
两个容器冒烟 job 都**不**等于真机验证 —— 容器通常缺语言栏/TSF。
MSI 的 `OpRegisterTsf` 失败/回滚会如实留在日志里；EXE 的注册失败只记日志、安装照常成功。
**2026-10-31 起** MSI 冒烟把「恰好是 `OpRegisterTsf` 回滚（1603 + 动作 Return value 3）」
视为 Server Core 无 TSF 的已知环境限制并通过；其他安装失败仍然红，不再是 continue-on-error。
「表里注入了」与「Windows 端能用」仍是两件事（§6）。

| artifact | 内容 | 实测体积（来源 / 单位 / 日期） |
|---|---|---|
| `opi-tsf-dll-v<版本>` | `tsf_opi.dll`（zip 里就这一个文件；**仅供 desktop-msi / desktop-exe job 下载打包，不再作为用户入口发布**） | DLL 本体 778,240 B、上传 zip 331,390 B（2026-10-09 run，旧构建） |
| `opi-windows-exe-v<版本>` | **用户入口**：`opi-<版本>-windows-x64-setup.exe`（Inno Setup，`desktop-exe` job 产出）。内含 jpackage app image（`opi.exe` + `app\tsf_opi.dll` + `app\luna.opid` + 许可文本 + jlinked JRE），默认装到 `C:\Program Files\opi`；装完用 64 位 `regsvr32 /s` 注册 TSF，**失败只记日志、不回滚**；完成页只启动候选窗，不碰 Explorer | 49,119,340 B（2026-10-20 v1.17.0 run 的 artifact；词库随包后体积会变，以新 run 为准） |
| `opi-desktop-msi-raw-v<版本>` | jpackage **原始** `.msi`（zip 里一个文件）。**仅内部中间物**（YAML 注释标 internal）：没有注册动作，只供 `msi-postprocess` 下载；后处理成功后由该 job 删除，**正常 run 的最终 artifacts 里不应出现** | 上传的 **zip** 58,683,236 B（2026-10-09 run）；**未压缩 MSI** 62,034,236 B（2026-10-19 本机核对的 v1.15.1 发布资产）。两者不是同一次构建，别混比 |
| `opi-msi-internal-v<版本>` | **后处理后的** `opi-<版本>.msi`（zip 里一个文件）。**内部证据，不是用户入口**（`msi-postprocess` job 产出；注册 CA 是 Type 1，失败会回滚） | **未压缩 MSI** 62,443,008 B（2026-10-19 本机由上一行的 v1.15.1 raw 处理得到；+408,772 B）。CI 上传的是 zip，别再拿这个数当 zip 体积 |
| `opi-msi-install-logs-v<版本>` | `windows-install-test` 在 Windows Server Core 容器里跑 `msiexec` 的日志与目录/注册表输出（`install-custom.log` / `install-default.log` / `dir-*.txt` / `reg-*.txt`）。容器缺语言栏/TSF，失败/回滚会如实留在日志里 | 每次 run 新生成 |
| `opi-exe-install-logs-v<版本>` | `windows-exe-install-test` 的静默安装/升级/卸载日志与断言输出（`install-exe.log` / `upgrade-exe.log` / `uninstall-exe.log` / `*-exit.txt` / `sha-*.txt` / `*-removed.txt`）。`regsvr32` 退出码在 `install-exe.log` 里，非 0 不判红；升级路径以 `sha-package.txt` / `sha-after.txt` 的 SHA-256 相等为判据 | 每次 run 新生成 |

名字里的版本来自 `Cargo.toml` 的 `[workspace.package] version`（本仓六 crate 共用的单一来源），
并与 `desktop/build.gradle.kts` 的 `opiAppVersion`（= jpackage 的 `packageVersion`）**比对**：
两处不等时工作流直接红 —— 不打出「名字与内容各说各话」的包。

**怎么拿**（Actions artifact 默认保留 90 天；**不自动附 Release**，与 `packages.yml`
「发版产物归人管」同一条约定）：

```bash
gh run list   --workflow=windows.yml                 # 找最近一次成功的 run
gh run download <run-id> -n opi-windows-exe-v<版本>  # 用户入口：opi-<版本>-windows-x64-setup.exe
gh run download <run-id> -n opi-tsf-dll-v<版本>      # 只有开发者要单独用 DLL 时
```

⚠️ **正常 run 的最终 artifacts 里不应出现 `-raw-`**：raw 只是
`desktop-msi → msi-postprocess` 的 job 间中间物，后处理成功后自动删除。若还能看到它，
说明后处理 job 失败（raw 保留用于排障，也不要安装）。用户唯一入口是
`opi-windows-exe-v<版本>`；`opi-msi-internal-v<版本>` 只是内部证据，不要发给用户。

**EXE 安装器（2026-10-20 起用户入口）**：脚本是 `scripts/windows/opi.iss`，
输入是 `createDistributable` 产出的 app image（`desktop\build\compose\binaries\main\app\opi`）。
安装向导的侧栏/小图使用 `desktop/icons/opi-pet-wizard.png` / `opi-pet-wizard-small.png`
（Inno Setup 6 支持 PNG；2026-10-21 用户裁决「安装过程中界面显示项目宠物图」）：

```powershell
cd desktop
$env:OPI_TSF_DLL = (Resolve-Path ..\target\x86_64-pc-windows-msvc\release\tsf_opi.dll).Path
.\gradlew.bat createDistributable --no-daemon
cd ..
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" `
  /DAppVersion=<版本> `
  /DAppImageDir="desktop\build\compose\binaries\main\app\opi" `
  /DOutputDir="dist\windows-exe" `
  scripts\windows\opi.iss
```

静默安装 / 卸载（`windows-exe-install-test` 用的就是这两条）：

```bat
opi-<版本>-windows-x64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
C:\Program Files\opi\unins000.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
```

- 默认目录 `C:\Program Files\opi`（`DefaultDirName={autopf}\opi`，可 `/DIR=` 覆盖）；
  安装前会 `taskkill /f /t /im opi.exe` 关掉运行中的旧实例（升级用，失败不拦）。
  候选窗与状态栏各有一把 `%TEMP%\opi-candidate-<用户>.lock` /
  `opi-statusbar-<用户>.lock` 单实例锁（2026-10-25 加候选窗锁，重复启动的
  `opi.exe` 会自己退出，不再堆出多个常驻进程）。
- 文件落盘后 `[Code]` 调 **64 位** `C:\Windows\System32\regsvr32.exe /s
  "{app}\app\tsf_opi.dll"`；结果只写日志
  `opi: regsvr32 /s tsf_opi.dll exit code <n>`，**非 0 也不回滚**。卸载时先结束
  `opi.exe`、再 `regsvr32 /s /u`、再删文件（同样 best-effort）。
- **任务栏输入指示器图标（2026-10-21）**：`RegisterProfile` 的 icon 指向安装目录根的
  `opi.exe`（jpackage 宠物图标），不再传空图标；升级安装会先 `regsvr32 /u` 旧 DLL 再
  注册新 DLL，以便已存在的 profile 也换上新图标。**图标不是已证实的根因**：带图标的
  1.20.0 提交在 Server Core 冒烟里 `regsvr32` 退出码 0、安装通过，用户那台机器的
  差异无法在 CI 复现。已确认的是几条**静默失败链**：图标失败会直接放弃 profile、
  `EnableLanguageProfileByDefault`/类别失败会中止后续注册、安装器只看退出码不核
  CTF 键、升级时 `UnregisterTsf` 的摘除窗口跨越整个文件复制阶段。现在图标失败会
  自动用空图标重试，重试仍失败但 profile 已能读回也算成功；其他降级步骤写日志，
  不再静默。真机上的任务栏显示仍待验证。
- **卸载（2026-10-21，同日真机「目录残留 / tsf_opi.dll 被占用」报障后修）**：
  `CurUninstallStepChanged(usUninstall)` 在 Inno 删文件前先
  `taskkill /f /t /im opi.exe`（候选窗 / 设置窗 / 状态栏都是同一个 `opi.exe`；
  **本树没有 Windows 服务**），随后清 HKLM64/HKCU Run 下的 `opi` / `opi-statusbar`
  值与 `%TEMP%\opi-statusbar-<用户>.lock`（状态栏单实例锁：进程被杀后文件仍在）。
  `[UninstallDelete] filesandordirs "{app}"` 清整个安装目录。**不杀 Explorer，也不再
  请求重启**：被 Explorer/TSF 宿主映射的 DLL 不能删除（映像映射），但加载器以
  `FILE_SHARE_DELETE` 打开它，**允许改名**。`MoveLockedTsfDllOut` 先试普通删除；失败
  则用 `MoveFileExW` 改名到**同卷**的 trash 目录：`{commonappdata}\opi\trash`
  （应用装在系统盘时）或 `<盘符>:\opi-trash`（装在其他盘时），文件名是唯一的
  `opi-tsf-<时间戳，精确到毫秒>.dll`。`%TEMP%` 只在**同卷**时才会用
  （`ExtractFileDrive` 比较；用户把 TEMP 放到 RAM 盘/别的盘时，跨卷
  `MoveFile` 必失败 `ERROR_NOT_SAME_DEVICE`），再退到 `{win}\Temp`。
  改名成功后 `{app}` 可立即删除；副本用
  `MoveFileExW(路径, '', MOVEFILE_DELAY_UNTIL_REBOOT)` 登记下次开机删除，不弹窗。
  下次安装/卸载会先清一遍 trash（仍被映射的副本留给开机删除）。
  **旧版为什么失败**：目标名是固定的 `%TEMP%\opi-tsf-uninstall-<用户>.dll`。上一次卸载
  留下的副本仍被映射 ⇒ `DeleteFile` 清不掉，而 Inno 的 `RenameFile` 在目标已存在时
  直接返回 False；跨卷时 `MoveFile` 也失败。两种情况下 DLL 都留在 `{app}`，Inno 的
  删除失败后只能报 "Some elements could not be removed"，用户再手动删就看到
  「文件被占用」。最后兜底（改名仍失败）：`MoveFileExW(原路径, '', MOVEFILE_DELAY_UNTIL_REBOOT)`
  登记开机删除并**如实**写日志 `{app} remains until then`，不重启、不谎称已清理；此时交互式卸载仍会显示
  Inno 的 "Some elements could not be removed"（目录确实没删干净，见 §6.1 第 15 条）。
  未占用时走普通删除。
  **没有用 Restart Manager**：Inno 的 `CloseApplications` 只作用于安装阶段；卸载器
  里自建 RM 只能 `RmShutdown` 终止持有 DLL 的进程（通常是 Explorer / 前台应用），
  正是 2026-10-20 白屏的同一类动作，而改名移出已经能立即清掉目录。
  EXE 容器冒烟现在会**真的把 DLL 映射住**再卸载，断言 `C:\opi-test` 消失且日志走
  「moved locked TSF DLL」路径；真机 Explorer 的持有方式仍只能真机复验。
- **卸载残留清理（2026-10-21 真机测试后补）**：`DllUnregisterServer` 只摘 TSF 官方
  API 管理的键，Windows 仍保留三处每用户 UI 状态 —— 键盘列表值
  `HKCU\Control Panel\International\User Profile\<语言>\0804:{CLSID}{PROFILE}`、
  输入法排序 `HKCU\Software\Microsoft\CTF\SortOrder\AssemblyItem\...`（按 `CLSID`
  值识别）、`HKCU\Software\Microsoft\CTF\TIP\{CLSID}` 启用子树。卸载器新增
  `RemoveTsfRegistryResidue` / `RemoveUserInputMethodTip` / `RemoveSortOrderEntries`，
  在 `usUninstall` 清理；**不放进 `DllUnregisterServer`**：升级时的 `regsvr32 /u`
  会把用户键盘列表条目一起删掉，而 `RegisterProfile` 不会重新加回，升级用户会
  发现输入法从列表消失。DLL 已缺失时 `UnregisterTsf` 提前返回，上面的注册表
  兜底仍会执行。`usPostUninstall` 再清一次 trash 并删除空目录。
  **用户数据默认保留**：`%LOCALAPPDATA%\opi`（`settings.ini`、`dicts\`）是用户数据，
  不是安装残留；需要全清时用
  `unins000.exe /VERYSILENT /DELETEUSERDATA`（静默脚本/干净机器）。
  真机验证（2026-10-21，Windows 10 Home）：正常卸载、映射 DLL + 运行中候选窗、
  缺失 DLL、`/DELETEUSERDATA` 共 89/89 项通过；日志仍为
  `Removed all? Yes` / `Need to restart Windows? No`，Explorer PID 未变。
- **升级路径（2026-10-21，「装完 1.20.1 仍只有英文」的追加根因）**：`[Files]` 的
  `restartreplace` 只在**删除旧目标失败**时生效 —— Inno 把新 DLL 解到临时文件，
  `RestartReplace(临时文件, 目标)` 排进 `PendingFileRenameOperations`，然后**跳过**
  本应把临时文件改名到目标的 `MoveFile`（`Setup.Install.pas` 的
  `DoHandleFailedDeleteOrMoveFileTry`：置 `ReplaceOnRestart := True` 后
  `NextAction := naStopAndSucceed`）。旧 DLL 在重启前一直留在
  `{app}\app\tsf_opi.dll` ⇒ `ssPostInstall` 的 `regsvr32` 注册的也是**旧 DLL**，
  用户不重启就仍是旧 TSF 代码。现在 `CurStepChanged(ssInstall)` 在
  `[InstallDelete]`/`[Files]` **之前**先 `UnregisterTsf()` 摘掉旧 profile/COM 键，
  再 `MoveLockedTsfDll(True)` 把被映射的旧 DLL 改名到同卷 trash；新 DLL 当场落到
  原路径，`ssPostInstall` 的 `regsvr32` 注册的是新 DLL。
  **【订正 2026-10-21 真机】`restartreplace` 已删除**：升级时 `luna.opid` 也被旧
  TSF DLL 映射，Inno 把它排进 `PendingFileRenameOperations`（`Will replace on
  restart`），下一次安装的 `PrepareToInstall` 因该值**拒绝安装并要求重启** ——
  正是用户禁止的行为。现在 ssInstall 同时 `MoveLockedDictionary(True)` 把
  `luna.opid` 改名移出；`InitializeSetup` 在安装前清掉指向本 `{app}` 的陈旧
  pending 项（`/DIR` 或默认 `ProgramW6432\opi`）；改名失败则 `[Files]` 失败（响亮），
  绝不排重启。日志同时打印两个文件的大小（`... on disk: N bytes` / `installed: N
  bytes`）；大小相同不构成「已替换」的证据，CI 用包内 SHA-256 判定。
  ⚠️ 文件名换了不等于**运行中的**代码换了：Explorer/已打开应用里已加载的旧映像
  仍是旧代码。交互式升级会重启 Explorer（shell 立即换新），其它已打开的应用要
  关闭重开；**不需要注销**（见「安装后必做」第 1 步与 §4.3）。
- 快捷方式：开始菜单「opi settings」= `opi.exe --settings`；桌面图标在安装时可选。
- **登录自启（2026-10-20）**：`[Registry]` 写一条 **HKLM64**
  `SOFTWARE\Microsoft\Windows\CurrentVersion\Run` 值 `opi` = `"{app}\opi.exe"`，
  `uninsdeletevalue` 在卸载时删除。本脚本已声明 `ArchitecturesInstallIn64BitMode=x64`
  （普通 `HKLM` 本来就会走原生视图）；这里仍显式写 `HKLM64`，是为了不依赖安装模式的
  隐式行为 —— 将来改模式时不会静默落进 `Wow6432Node`（Explorer 只读原生视图）。
  TSF DLL 只是管道客户端，候选窗进程不在就没有候选 UI；HKLM64 与机器级 TSF 注册
  同一层级，且只有这一条（不是每用户一条）。
- **状态栏只在切到 opi 时显示（2026-10-21）**：`--statusbar` 仍随登录/完成页启动，
  但 `barVisible(..., requireActive = isWindows())` 要求控制面读到 `im=opi`；TSF
  未连接/切换到别的输入法时读不到状态 ⇒ 条隐藏，避免安装完就出现一条点哪个键都报
  「连不上输入法」的条。切回 opi 后 1s 轮询让它出现。
- **完成页（2026-10-20 白屏修复后）**：可勾选拉起 `launch-and-restart.cmd`。脚本
  只用 `tasklist` 查重（已有 `opi.exe` 就不重复启动）并 `start` 候选窗，**不碰
  Explorer**。旧脚本会 `taskkill /f /im explorer.exe`，再用
  `ping -n 1 -w 200 127.0.0.1` 轮询 shell：loopback 的 ping 不等待，8 次轮询可在
  毫秒级跑完，`tasklist` 还可能匹配到正在退出的 Explorer，于是脚本在 shell 回来前
  就退出；回退的 `start explorer.exe` 又会与 Winlogon 的自动重启抢。更关键的是
  `[Run]` 原先写的是 `runascurrentuser`：Setup 本身是提权的，这个 flag 让脚本拿到
  **管理员令牌**（Inno 的 `postinstall` 默认是 `runasoriginaluser`，原注释把两者说反了），
  于是 `taskkill` 是从管理员进程杀掉用户 shell，回退再以管理员令牌拉起一个**提权
  Explorer** —— 它不是用户的桌面 shell，任务栏/桌面可以一直白着。现已改为
  `runasoriginaluser`，脚本也不再碰 Explorer。**Shell 的受控重启不在完成页脚本里**：
  它在 `ssPostInstall` 的 `RestartExplorerAfterInstall`（原用户令牌 + 看门狗，每次
  交互式升级都执行）；完成页文案统一为「无需注销；新打开的应用生效，已打开的应用
  关闭重开；静默安装不重启 Explorer」。
  静默安装跳过完成页 ⇒ 自启仍由 HKLM Run 在下次登录生效。**【未验证】**：新脚本的
  交互路径没有在真机跑过（CI 的 `/VERYSILENT` 会跳过 `[Run]`）。
- **`CloseApplications=no`（2026-10-20）**：jpackage/Inno 的 Restart Manager 默认会
  关闭「占用待替换文件」的应用；`tsf_opi.dll` 可能正被 Explorer 加载，升级时关掉
  Explorer 后就可能不再回来。这里显式禁用 Restart Manager；被占用的 DLL 与
  `luna.opid` 由 ssInstall 的改名清锁（见上一条）。`restartreplace` 已删除
  （2026-10-21 真机：它会把文件排进 `PendingFileRenameOperations`，下一次安装
  因此拒绝并要求重启）；改名失败时安装响亮地失败，绝不排重启。
  `RestartApplications=yes` 仅作第二道保险（`CloseApplications=no` 时无效果，
  防止将来有人改回去）。
- 冒烟（`windows-exe-install-test`）：在干净的 Server Core 容器里静默装到
  `C:\opi-test`，断言 `opi.exe`、`app\tsf_opi.dll`、`app\luna.opid` 存在、安装退出码 0、
  日志里**有** `regsvr32` 退出码（0 或非 0 均可）；2026-10-21 起还断言
  **HKLM CTF 语言 profile 键真的写入**、**`opi-statusbar` 的 HKLM Run 值存在**；
  同日起还验证**升级路径**：先给已安装的 DLL 尾部追加一个字节（哈希与包内不同），
  用只 `LoadLibrary` 的进程把它**映射住**，再跑一次**同一个安装包**，断言磁盘上的
  `tsf_opi.dll` SHA-256 **变回包内版本**、升级日志有 `moved locked TSF DLL` 且
  没有 `Will replace on restart`（只断言退出码会假绿：同版本重装的退出码本来就是 0）。
  然后重新起一个占用进程，走卸载路径：静默卸载并断言 `opi.exe`、**整个
  `C:\opi-test` 目录**、被占用的 DLL 以及上面两个注册表项都消失；卸载前还会
  种入一条每用户键盘列表值和一个输入法排序项（模拟用户在设置里添加过该输入法），
  卸载后断言 `HKCU\Control Panel\International\User Profile` 下的值、HKCU CTF TIP
  键与排序项都不在了（2026-10-21 真机残留修复的回归门禁）。卸载日志里还须有
  `moved locked TSF DLL`、`Removed all? Yes`，没有 in-use 删除失败。容器里这个
  占用者不是 Explorer，真机 Explorer/TSF 宿主的持有方式仍需真机复验。
  **退出码 0 不是 profile 写入的证据** —— 这正是 2026-10-21
  真机「装完仍只有英文」要区分的两件事。「TSF 真加载了词库」仍要真机验证。
  ⚠️ 静默路径不执行完成页脚本，该脚本由 `ci.yml` 的静态门禁守护，不是 CI 跑过的；
  静默安装也**不启动**状态栏进程（只有 Run 值），只能证明「下次登录会启动」。

#### 安装后必做（2026-10-21 真机报障后补）

装完**不会立刻**出现可用输入法，按顺序做这三步；缺任何一步的表现都是
「输入只有英文、切换键没反应、状态栏不出现」：

1. **安装完成不要求注销**。统一口径（2026-10-23）：
   - 用户入口 EXE 的**交互式升级**会在注册新 DLL 后，以**原用户令牌**重启该用户的
     Explorer（看门狗等待 3 秒，必要时原用户令牌补启；桌面/任务栏会短暂消失后回来），
     **不需要注销**。重启判定是「Explorer 是否映射旧 DLL」；探测不到就不碰 shell（白屏回归）。
   - **首次安装、静默安装**不重启 Explorer（静默安装同样跳过完成页）。
   - **新打开的应用**加载新 DLL，立即生效；**已打开的应用**仍持有旧映像，需关闭后
     重新打开（完成页会列出仍加载旧 DLL 的进程）。本安装器**不会**自动结束这些应用
     （未保存数据风险），也不承诺对已运行进程热替换。
   - 完成页/日志的固定文案：无需注销；新打开的应用生效，已打开的应用关闭重开；
     静默安装不重启 Explorer。
   **2026-10-22 真机补充**：怀疑「装了新包却没生效」时先跑
   `scripts\windows\diagnose-tsf.ps1` —— 它按注册表自动定位安装目录，并列出所有仍映射
   `tsf_opi.dll` 的进程。`[WARN] stale host` = 该进程在安装前启动、加载的是旧映像
   （Chrome/Sublime/旧 Notepad 最常见；Explorer 由安装器负责重启）。Windows 不能替换
   已加载的 DLL，在这些进程里测试只会复现旧行为（切输入法状态栏不消失、新应用连不上
   候选管道），与安装包本身无关；关闭重开该应用后才换上新代码。
2. **把输入法加进键盘列表**：设置 → 时间和语言 → 语言和区域 →
   **中文(简体，中国)** → 语言选项 → 键盘 → 添加键盘 → **小欧输入法**。
   系统里没有「中文(简体，中国)」语言时，先添加该语言。注册用的 LANGID 是 zh-CN，
   所以它只出现在中文键盘列表里。
3. 用 `Win` + `Space` 切到「小欧输入法」。**状态栏只在切到小欧后显示**
   （`barVisible(..., requireActive=isWindows())` 读控制面的 `im=opi`）；还在英文/别的
   输入法下时状态栏隐藏是预期行为，不是安装失败。

**注册失败会当场可见（2026-10-21）**：交互安装时，`regsvr32` 非 0 或 HKLM 语言
profile 键没写出来，完成前会弹窗（日志路径 + 重试指引）；静默安装只写日志。
固定日志：`%ProgramData%\opi\tsf-register.log`（每次注册前清空；记录致命与降级步骤的
HRESULT），与 Inno 安装日志里的 `opi: regsvr32 ... exit code <n>` 行配合看。
`scripts/windows/diagnose-tsf.ps1` 会读这份日志。

或在 Actions 页面（仓库 → Actions → Windows 打包）点进某次 run 下载。
**触发**：手动 dispatch，或 push 到默认分支且改动落在 `crates/tsf-opi/**` / `desktop/**` /
`android/app/src/main/assets/luna.opid` / `scripts/postprocess-windows-msi.sh` /
`scripts/windows/**` / `.github/workflows/windows.yml`。
⚠️ 本仓约定 workflow
要**先存在于默认分支**才能在 Actions 页面手动 dispatch —— 合并前它只能被分支 push 触发。

**那次 run 里真发生的事（【CI 实测】：`windows-latest`，run 37934999798，全绿；
https://github.com/erikwang2013/opi/actions/runs/37934999798 ）**：

1. `cargo build --release --locked -p tsf_opi --target x86_64-pc-windows-msvc` ——
   **本仓第一次在 Windows 目标上跑完整 release 链接**，产出 `tsf_opi.dll`。
   链接器给 4 条 `LNK4104`（"export of symbol 'Dll…' should be PRIVATE"）：那是 cdylib 导出
   这四个 COM 入口的常态建议，不是错 —— 下一条的导出表就是「它没挡住导出」的证明。
   **2026-10-19 起加 `RUSTFLAGS=-C target-feature=+crt-static`**：MSI 不安装 VC++ 运行时
   （jpackage 的 `runtime\bin` 在包内但不在 DLL 旁的 `app\`，也不进 PATH），动态 CRT 的
   DLL 在没装 VC++ 的机器上会让 `regsvr32` 以 `0x8007007E`（找不到模块）失败。静态链接后
   导入表里不再出现 `VCRUNTIME140.dll` / `MSVCP140.dll` / `ucrtbase.dll` /
   `api-ms-win-crt-*.dll` —— 紧接着的 `/dependents` 门禁只要出现其中任一模式就红
   （2026-10-19 reviewer m2 扩了后两类；`api-ms-win-core-*` 是系统 DLL，不在判据里）。
2. `dumpbin /exports` 读导出表（**§6.1 第 2 条的消除证据**）：

   ```
       ordinal hint RVA      name
             1    0 0000D4A0 DllCanUnloadNow = DllCanUnloadNow
             2    1 0000D4B0 DllGetClassObject = DllGetClassObject
             3    2 0000D9E0 DllRegisterServer = DllRegisterServer
             4    3 0000E6A0 DllUnregisterServer = DllUnregisterServer
   ```

   ⇒ 四个 COM 入口都在，**名字未被装饰**（`regsvr32`/COM 要的就是这四个原名）。
   少一个、或名字被装饰，工作流直接红（判据写在 YAML 里）。**RVA 每编一次都会变**，别抄那列。
3. `./gradlew.bat packageMsi --no-daemon`（temurin **21**，jpackage）—— 产出
   `opi-candidates-1.7.4.msi`。WiX **不用预装**：CMP 自己在 windows 主机上拉
   `wix311-binaries.zip`（日志里两步：`downloadWix` → `unzipWix`）。Kotlin 编译有 4 条既有告警
   （`Protocol.kt` 的 elvis 恒取左操作数），不挡构建。
4. 两个 artifact 都用 `if-no-files-found: error` 上传 —— 「绿了但没产物」会直接红（本仓栽过）。

**.msi 里是什么**（**【推断】**：读 `desktop/build.gradle.kts` 的配置 + jpackage 的产物结构，
**没有**开箱验证过）：

- 候选窗应用本体（`desktop/` 编出的 jar）+ **jlink 出来的 JRE 运行时**（58 MB 主要在这）+ 图标；
- **`tsf_opi.dll`**（2026-10-18 起）：`desktop-msi` job 先下载 `tsf-dll` job 的产物，经
  `OPI_TSF_DLL` 挂进 jpackage 的 `--input`，落在 `INSTALLDIR\app\`（后处理后的默认
  安装目录是 `C:\Program Files\opi`）。单独发布的 DLL artifact 从此**不再是用户入口**
  （用户裁决「DLL 不要单独发布，用户不会用」）；
- **`launch-and-restart.cmd`**：完成页复选框拉起的脚本（`tasklist` 查重后只启动
  候选窗；2026-10-20 白屏后不再结束/重启 Explorer，见 §2.5）；
- **`luna.opid`（2026-10-20 起）**：入库副本由 `desktop/build.gradle.kts` 的 JPackage
  `files` 集合放进 app image 的 `app\`，与 `tsf_opi.dll` 同目录（§2.4）；同批还有
  三份许可文本。不设置词库环境变量；
- ~~PUA 字体 `opi-emoji.ttf` + `install-emoji-font.ps1`~~：2026-10-20 用户裁决删除
  字体通道，MSI 不再包含这两份文件；自有表情只在候选窗内复制图片。

**图标（2026-10-19 实测于 v1.15.1 raw MSI）**：`Property` 表
`ARPPRODUCTICON=JpARPPRODUCTICON`，`Icon` 表同名行指向 `Icon.JpARPPRODUCTICON`
流；该流是 798,720 B 的启动器 PE（与包内 `opi-candidates.exe` 同尺寸），
`wrestool` 从中取出 6 个尺寸（256/128/64/48/32/16），逐帧 PNG 的 SHA-256 与
`desktop/icons/opi-pet.ico` **完全相同**。启动器 exe 自身的 icon 资源也与该 ICO
逐帧相同。⇒ MSI 产品图标与 launcher 图标都是 `desktop/icons/opi-pet.ico`（小欧
头像），不是 Java 默认图标。新构建沿用同一 `windows { iconFile.set(...) }`，但
**新产物仍应重做一次同样的逐帧比对**（构建工具可能换版本）。

**后处理（2026-10-17 / 10-18 加，2026-10-19 起由 CI 自动执行）**：CI 出的 `.msi` 只带
jpackage 的默认行为，而用户裁决了它做不了的四件事（「安装的时候中断正在运行的应用，
安装后启动」「覆盖安装新版本后还要重启才生效」「安装目录以 opi 为基础目录」）：

> **2026-10-19 的教训**：在 `msi-postprocess` job 出现之前，后处理只是文档里的**手工**
> 步骤，实际发布出去的正是 jpackage 原始 MSI —— 用户在真机装 v1.15.1 后键盘不显示，
> 根因就是**从未执行 `regsvr32`**。现在它是 CI 的必需步骤（`needs: [version, desktop-msi]`），
> 该 job 在 `ubuntu-latest` 上装 `msitools` 并跑脚本，再独立读一遍表做断言。

**本地重新处理旧 artifact / 排障**仍可用同一条容器命令（不覆盖输入时给第二个参数）：

```bash
# 在 debian:12 容器里跑（本机没有 msitools；同 scripts/build-packages.sh 的用法）
docker run --rm -v "$PWD:/w" -w /w debian:12 \
  sh -c 'apt-get update -qq && apt-get install -y -qq msitools >/dev/null \
         && scripts/postprocess-windows-msi.sh in.msi out.msi'
```

`scripts/postprocess-windows-msi.sh`（文件头有全部设计理由与「能证明/不能证明」）做四件事：

1. **安装目录**：`Directory` 表 `INSTALLDIR` 行的 DefaultDir 从 jpackage 的
   `...|opi-candidates`（2026-10-20 起 Windows 的 `packageName=opi`，原始行是
   `opi` 或 `short|opi`）改为 `opi` ⇒ 默认装到 `C:\Program Files\opi`。只改这一行，
   其余行原样整表导回；启动器名（`EXE`）由 MSI 的 `ProductName` 推出，现在是
   `opi.exe`。
2. 执行序列 797 加一条 `"taskkill" /f /t /im opi.exe`（引号是 WixQuietExec 的
   硬要求；排在 `RemoveExistingProducts@798` **之前** —— 否则运行中的旧实例会走
   「改名+重启后删」那条路，装完还活着）。
3. 执行序列 5890 先用 **Type 51（属性设置）** 动作把格式化后的命令行写进
   `WixSilentExec64CmdLine`；5900（InstallFiles 之后、InstallFinalize 之前）再由
   `WixCA` 的 **`WixSilentExec64`**（**Type 1，Return=check**；不是 65）执行
   `"[System64Folder]regsvr32.exe" /s "[INSTALLDIR]app\tsf_opi.dll"`，注册随 MSI 安装的 TSF COM 服务器。
   **2026-10-20 订正（CI Server Core 实测）**：Property 表的 Value **不会**被 MSI
   格式化；直接把 `[INSTALLDIR]`/`[System64Folder]` 写进 Property 会让 `QuietExec64`
   拿到字面量并返回 `0x80070003`，`OpRegisterTsf` 失败、安装回滚、安装目录为空。
   Type 51 动作的 Target 列才会格式化，且 5890 必须先于 5900。
   **必须用 64 位变体**（2026-10-19 复审 blocker）：随包的 `Binary.WixCA` 是
   **PE32 i386**（216,496 B），裸 `WixSilentExec` 在 WOW64 里解析到 SysWOW64 的 32 位
   `regsvr32`，加载不了 x86-64 的 `tsf_opi.dll`。`WixSilentExec64` 配 `WixSilentExec64CmdLine`，
   命令与 `regsvr32` 都走 64 位。**注册失败会让安装本身失败并回滚**（Type 65 的
   Continue 位会把失败吞成「装成功、键盘不出现」，2026-10-19 reviewer M2）；排障看
   `msiexec /i <包> /l*v install.log` 里的 `OpRegisterTsf`。条件 `NOT REMOVE`：安装/修复执行，
   卸载不执行（卸载不清理注册的后果见 §6.1 #13）。
   `WixSilentExec64` 用自己的 `WixSilentExec64CmdLine` 属性 —— 不复用 797 的
   `WixQuietExecCmdLine`（同一属性两条命令会互相踩）。
   `WixCA` **没有** `WixShellExec64`（导出表实证），所以第 4 条的完成页自启仍是 32 位
   `WixShellExec`；32 位 ShellExecute 同样能拉起 `.cmd`。
4. ExitDialog 的 WixUI 自带复选框（默认勾选）在 Finish 时经 `WixCA` 的 `WixShellExec`
   只拉起 `launch-and-restart.cmd` 启动候选窗，**不结束/重启 Explorer**。
   `WixShellExec` 从提权的 msiexec 派生，所以这个候选窗也是提权进程（MSI 只是
   内部证据；EXE 用户入口用 `runasoriginaluser`）。旧脚本若重启 shell 会拿到
   管理员令牌；2026-10-20 白屏后该路径已删除。

   > **2026-10-20 订正（两次）**：本段早先写的「只杀不 start，由 Winlogon 拉回」与
   > 稍后的「tasklist 查重 → 杀 Explorer → 约 3 秒探测 → start 回退」都是白屏前的
   > 旧脚本。后者的 `start` 回退在提权令牌下会拉起**管理员 Explorer**；Inno 的
   > `[Run]` 还用了 `runascurrentuser`，把「以登录用户运行」写反（Setup 本身提权）。
   > 现在完成页脚本与 MSI 路径都不碰 shell：脚本只启动 `opi.exe`，Inno 的 `[Run]`
   > 用 `runasoriginaluser`，`opi.iss` 用 `CloseApplications=no`。EXE 用户入口的受控
   > shell 重启不在完成页脚本里，而在 `ssPostInstall`（原用户令牌，仅在 Explorer 映射旧 DLL 的交互式升级
   > 执行，见「安装后必做」第 1 步）。防回归的静态门禁在 `ci.yml` 的
   > `packaging-scripts` job（受控路径除外，拒绝 shell 结束命令与 `runascurrentuser`）。

复选框文案因此变成 `Launch opi`（由 `ProductName` 生成），完成页还有一行说明
（`WIXUI_EXITDIALOGOPTIONALTEXT`）：**MSI 是内部证据包**，不重启 shell，输入法在
下次注销/登录后才加载，其它已打开的应用要各自重启才换到新版本。
**用户入口 EXE 不走这条路径**：交互式升级会重启 Explorer，安装完成不要求注销，
统一口径见「安装后必做」第 1 步。脚本自带前置断言（槽位空、基线行在）、**显式后处理标记**
（Property `OPI_POSTPROCESSED=<形状版本>`：带同一标记的重跑直接退出 0 —— 含**不含
`tsf_opi.dll`** 的旧/测试产物，它们会跳过注册但同样算已处理，2026-10-19 reviewer m3）、
迁移护栏（无标记的旧产物一律拒绝，须在原始 artifact 上重跑，不会静默追加）与
**读回 + 全表 diff** 完整性检查（含 `INSTALLDIR=opi`），任一不满足即不落产物。
**旧 CI artifact 不含 DLL/脚本**：脚本会打印提示并跳过注册（回退为直接启动 exe），
不会假装注册成功；这样的产物同样带标记，重复处理是幂等的。

**本地已处理的 v1.15.1 MSI：仅供表级复核，不可发布**。
`dist/opi-1.15.1_windows-desktop-processed.msi` 是本机用修订后的脚本处理出来的
（2026-10-19：注册 CA 为 Type 1/`WixSilentExec64`、带 `OPI_POSTPROCESSED=2026-10-19-r3`
标记），表级动作齐全；原始 jpackage 产物未动。但包内 DLL 是**旧构建（动态 CRT）**，
导入表里有 `VCRUNTIME140.dll` 与 `api-ms-win-crt-*` ⇒ 只能在**装过 VC++ 运行时**的机器上
注册成功；desktop jar 也是修复前的构建。**不要把它当可安装交付物发布或拿去验收**；
需要真机验证请用 CI 的 `opi-windows-exe-v<版本>`（用户入口；静态 CRT、含本轮修复）；
要复核 MSI 表形状则用 `opi-msi-internal-v<版本>`（内部证据）。

**本地静态 CRT CAB 重打包：已撤回，不是交付物（2026-10-19 晚）**。曾用 Linux
`gcab` 重打 `Data.cab`、替换 File 键
`filedfd5da438ec03d8391aeff2429bdad72` 并重跑后处理，产出过
`dist/opi-1.15.1_windows-desktop-processed-static.msi`。它只做过 `msiextract` /
表级读回（195 个文件、DLL 哈希与导入表、四个导出、幂等），**从未被 `msiexec`
实际安装过**；Linux `gcab` 产出的 cab 不满足「可交付」的证据档位，故撤回该文件与
把它当测试包的用法。需要静态 CRT 的真机包，只能从 CI 的 `opi-windows-exe-v<版本>`
（用户入口）或 §2.6 的 Windows 本机构建取得；MSI 路线用 `opi-msi-internal-v<版本>`。

**本地 `processed` 包只作表级证据**：`dist/opi-1.15.1_windows-desktop-processed.msi`
（若本机还保留）是用修订后脚本处理 v1.15.1 raw 得到的，用来复核 `INSTALLDIR=opi`、
Type 1 的 `OpRegisterTsf@5900`、`WixSilentExec64` 与 `OPI_POSTPROCESSED` 标记。它的
**desktop jar 与 DLL 仍是旧构建**，不含 2026-10-19 之后的源码改动，也**没有**本轮新增的
注册修复（`RegisterProfile` 判存在、`EnableLanguageProfileByDefault`、删除旧
`AddLanguageProfile`）——
**不要拿它验收界面或输入法列表，也不要发布**。需要新界面/新注册逻辑时，
必须重新构建（§2.6）。

**升级后哪些进程会重载、哪些不会**（全部【未验证】：本机没有 Windows）：

| 对象 | 会发生什么 |
|---|---|
| 新启动的进程 | 加载磁盘上的新 DLL（MSI 的 File 表已含 `tsf_opi.dll`） |
| Explorer / shell / 语言栏 | **EXE 交互式升级且 Explorer 映射旧 DLL**：注册后用原用户令牌重启 shell（带看门狗），新 DLL 立即生效。首次/静默安装不重启。MSI 路径仍为 `CloseApplications=no`，下次注销/登录后重载 |
| 已打开的其它应用（Office / 浏览器 / 编辑器…） | **仍持旧 DLL**，直到各自关闭重开 |
| 候选窗进程 | 装前被 797 结束、装后由完成页脚本拉起 |
| COM 注册 | `OpRegisterTsf@5900` 经 `WixSilentExec64` 调 64 位 `regsvr32 /s` 注册 per-machine（Type 1：失败则安装失败/回滚）；卸载**不清注册**（`NOT REMOVE` ⇒ 不跑 `regsvr32 /u`，`HKCR`/`HKLM\SOFTWARE\Microsoft\CTF` 残留指向已删 DLL 的条目，见 §6.1 #13） |

- **静默 / 基本 UI 安装**（`msiexec /qn`、`/qb`）不出现 ExitDialog ⇒ 不拉起候选窗（HKLM
  Run 在下次登录补上）。**MSI 内部包**不结束 Explorer，也不需要用户手动 `taskkill`；
  要让 TSF 换到新 DLL，注销/登录或关闭重开应用。（用户入口 EXE 的口径见「安装后必做」
  第 1 步：交互式升级会重启 Explorer，无需注销。）
- **用户取消勾选** ⇒ 只跳过完成页自启，不会影响任何 shell 状态。
- **失败回退**：注册（`OpRegisterTsf@5900`，Type 1）**失败会让安装失败并回滚**（有意：
  宁可响亮失败，也不留「装成功、键盘不出现」的半注册状态）。完成页启动候选窗
  （`OpLaunchApp`，完成页 Finish 时触发、在 InstallFinalize 之后）失败则不影响安装结果。
- **shell 重启范围（2026-10-23 订正）**：EXE 的 `RestartExplorerAfterInstall`
  **只有 Explorer 映射旧 DLL 的交互式升级**才用**原用户令牌** `taskkill /f /im explorer.exe`，
  并由原用户令牌看门狗补启；未映射（包括 `tasklist /m` 探测不到）就不碰 shell，避免
  2026-10-23 的白屏回归。首次安装、静默安装、卸载都不碰 shell。旧版无条件的
  `taskkill` 会命中所有可访问会话、且首次安装也白重启，那一条仍已移除。
  **代价**：升级瞬间所有文件资源管理器窗口会关闭；其它已打开的 TSF 宿主（Word/浏览器）
  仍持旧 DLL，需各自关闭重开（完成页/日志会列出这些进程）。
- ✅ **2026-10-18 起 MSI 含 `tsf_opi.dll`**：File 表由 CI 的 `desktop-msi` job 在打包时
  写入（`OPI_TSF_DLL` → jpackage `--input`），后处理只负责注册动作与完成页脚本。
- ⚠️ **注册路径按 jpackage app-image 布局推定为 `INSTALLDIR\app\tsf_opi.dll`**
  （Linux 同名布局是 `lib/app/`），后处理默认 `INSTALLDIR` = `C:\Program Files\opi`；
  Windows 真机上装一次前，这个路径没有被实际执行过。

**这条改动的证据档位**（照实）：表级形状在本地可复核（读回断言全绿 + 除 Property /
CustomAction / InstallExecuteSequence / ControlEvent / Directory 五张表外全库逐字节未变；
⚠️ 这是**表结构**证据 —— `msiinfo tables` 不列 `_Streams`/cab，脚本另做 `msiextract`
**抽样解包**（能解出启动器与 DLL），不是 cab 逐字节比较，2026-10-19 reviewer m1）；
四个入口点在**这份 MSI 自己的** `Binary.WixCA`（3.11.2.4516）里实测存在（容器内
`objdump -p` 读导出表：`WixShellExec` / `WixQuietExec` / `WixSilentExec` /
`WixSilentExec64`；`strings -el` 读到 `WixSilentExec64CmdLine`）。同一二进制的
**PE machine 也是实测的**：`objdump -f` 报 `pei-i386`（PE32 i386，216,496 B）——
这正是注入必须用 `WixSilentExec64` 的原因（32 位 CA 会调 SysWOW64 的 regsvr32）。
CI 的 `msi-postprocess` job 现在独立断言这两点（PE machine 必须是 i386 + 导出表必须有
`WixSilentExec64`），不再只查表形状。DLL 是否真的随包落到 `INSTALLDIR\app\`、
`regsvr32` 是否真的注册成功，只能在 Windows 真机上装一次来验。**「在真 Windows 上装一遍」仍然一次都没有**
（§6.1 #12）—— `taskkill opi.exe` 杀没杀掉、注册没注册、完成页启没启起来，都还没人见过。

**这个 artifact 明确不保证什么**（别读成「Windows 端能用了」）：

- DLL **在本机只验证到「随 jpackage app-image 落位」**（Linux 同布局）；Windows 真机上的
  `regsvr32` 与宿主加载**从未发生**（§3.1 仍保留手工注册命令作为核对/回退）；
- 「文本插入真的落进文档」**从未验证**（§5.3、§6.1 #5）；
- .msi **未签名**：装的时候 Windows 会有来源警告（SmartScreen），这是现状不是故障；
- Windows 上**没有人装过、跑过**这两个产物 —— §6 里所有【未验证】条目都不因这次打包而消失。

**一条 Windows 侧硬约束（踩出来的，写在这里省一次排查）**：`description` / `vendor` /
`packageName` 必须是**纯 ASCII**。jpackage 把 `--description` 写进 **MSI 数据库**，而数据库
代码页由 WiX 本地化文件决定：jpackage 内置的 en-us 那份写死 **1252**（Windows Installer
不支持 65001/UTF-8），中文描述会让 `light.exe` 以 **LGHT0311** 退出 —— **实测退出码 311**，
且 Gradle 只转述「Exit code: 1」，真报错在 `desktop/build/compose/logs/packageMsi/*.txt`
（工作流里有一条 `if: failure()` 的诊断步骤专门把它打出来）。
现在 `description` 是 ASCII 的 `OPI IME candidate window (TSF plugin over a named pipe)`
（`.deb` 的 `Description` 跟着一起变英文 —— CMP 的 DSL 只有**一处** `description`，没有按平台
分设的接口）；工作流里另有一条前置守卫，这些值一旦出现非 ASCII 直接红。
想在 Windows 上要中文文案：自带 `MsiInstallerStrings_*.wxl` 走 `--resource-dir`（那份的
Culture 不能是 en-us），需要连 jpackage 选的 locale 一起换 —— en-US 的 runner 上做不到。

### 2.6 在 Windows 本机从零打包（2026-10-19，2026-10-20 起 EXE 优先）

> **结论先说**：用户入口是 **EXE 安装器**，由 jpackage app image +
> **Inno Setup 6** 打包（下面的「EXE 路线」）。jpackage 的 `.msi` **只能在 Windows
> 上打**，且**必须**过后处理（§2.5）才会注册 TSF；2026-10-20 起它只是内部证据，
> 不要发给用户。Linux 上的 `cargo xwin build` 只能验证 TSF DLL 的交叉链接；
> 也不要用 Linux `gcab` 重打 CAB 当交付物（§2.5 已撤回）。

**前提（Windows 10/11 x64）**：

- **JDK 21**（Temurin；`JAVA_HOME` 指向它），Gradle 用仓库自带的 `gradlew.bat`；
- **Visual Studio Build Tools**，勾选「使用 C++ 的桌面开发」（MSVC x64 + Windows SDK）
  以及 `dumpbin`；
- **Rust stable MSVC** toolchain（`rustup default stable-x86_64-pc-windows-msvc`）；
- **Inno Setup 6**（`ISCC.exe`，默认在 `C:\Program Files (x86)\Inno Setup 6\`）——
  只有打 EXE 需要；
- **WiX 3.11 不用预装**：Compose Multiplatform 在 `packageMsi` 时自己下载
  `wix311-binaries.zip`（日志里能看到 `downloadWix` / `unzipWix`）；
- 打包机需要能访问 Maven/Gradle 与 WiX 下载源；离线环境请先预热 Gradle/WiX 缓存。

**第 1 步：编译静态 CRT 的 TSF DLL**

```powershell
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --release --locked -p tsf_opi --target x86_64-pc-windows-msvc
```

检查导入表与导出表（**这一步不能省**：MSI 不装 VC++ 运行时，动态 CRT 会让
`regsvr32` 以 `0x8007007E` 失败）：

```powershell
dumpbin /dependents target\x86_64-pc-windows-msvc\release\tsf_opi.dll
# 不能出现 VCRUNTIME140 / MSVCP140 / ucrtbase / api-ms-win-crt-*
dumpbin /exports target\x86_64-pc-windows-msvc\release\tsf_opi.dll
# 必须看到 DllCanUnloadNow / DllGetClassObject / DllRegisterServer / DllUnregisterServer
```

> 本机（Linux）的等价验证命令是
> `RUSTFLAGS='-C target-feature=+crt-static' cargo xwin build --release --locked -p tsf_opi --target x86_64-pc-windows-msvc`。
> 2026-10-19 实测：普通构建成功；静态构建也成功，导入表只剩
> `advapi32` / `api-ms-win-core-synch-l1-2-0` / `bcryptprimitives` / `combase` /
> `kernel32` / `ntdll` / `ole32` / `oleaut32` / `user32`，四个 COM 导出齐全。

**EXE 路线（用户入口，2026-10-20 起）**

同一个 `OPI_TSF_DLL` 先打 app image，再用 Inno Setup 编译：

```powershell
$env:OPI_TSF_DLL = (Resolve-Path target\x86_64-pc-windows-msvc\release\tsf_opi.dll).Path
cd desktop
.\gradlew.bat createDistributable --no-daemon
cd ..
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" `
  /DAppVersion=<版本> `
  /DAppImageDir="desktop\build\compose\binaries\main\app\opi" `
  /DOutputDir="dist\windows-exe" `
  scripts\windows\opi.iss
```

产物是 `dist\windows-exe\opi-<版本>-windows-x64-setup.exe`；静默安装、TSF 注册（best-effort）
与卸载语义见 §2.5。下面的第 2–4 步只适用于 **MSI 内部证据路线**。

**第 2 步：打 desktop MSI（内部证据）**

```powershell
cd desktop
$env:OPI_TSF_DLL = (Resolve-Path ..\target\x86_64-pc-windows-msvc\release\tsf_opi.dll).Path
.\gradlew.bat packageMsi --no-daemon
```

`OPI_TSF_DLL` 必须在 `packageMsi` 前设置：`desktop/build.gradle.kts` 只在
该变量存在时把 DLL、`launch-and-restart.cmd`、入库词库
`android/app/src/main/assets/luna.opid` 与三份许可文本挂进 jpackage 的 `--input`
（后两者 2026-10-20 起；词库落在 `app\luna.opid`）。产物通常在
`desktop\build\compose\binaries\main\msi\opi-<版本>.msi`
（Windows 主机上 `packageName=opi`；CMP 版本可能改路径，先
`Get-ChildItem -Recurse -Filter *.msi` 确认）。

**第 3 步：后处理（必需）**

**首选：Windows 原生 PowerShell**（本仓库 2026-10-19 新增，使用系统自带的
`WindowsInstaller.Installer` COM，不依赖 msitools/WSL）：

```powershell
# 回到仓库根目录（第 2 步 cd 进了 desktop）；以管理员身份运行 64 位 PowerShell
cd ..
powershell -ExecutionPolicy Bypass -File scripts\windows\postprocess-windows-msi.ps1 `
  -InputMsi desktop\build\compose\binaries\main\msi\opi-<版本>.msi `
  -OutputMsi dist\opi-<版本>_windows-desktop-processed.msi
```

它做与 `scripts/postprocess-windows-msi.sh` 相同的五张表修改（`INSTALLDIR=opi`、
797 杀进程、5900 Type 1 `regsvr32`、Finish 自启、`OPI_POSTPROCESSED` 标记），
并在 `Commit` 前做表级读回。⚠️ **该脚本尚未在真 Windows 主机上执行过**
（仓库的验证环境是 Linux），所以它是「本机打包的便捷路径」，不是已验证交付路径；
失败时不要退回原始 MSI 发布，改用下面这条路。

**已验证路径（CI/WSL）**：在 WSL/Debian 里装 `msitools` 后跑
`scripts/postprocess-windows-msi.sh`（命令见 §2.5）；CI 的
`opi-msi-internal-v<版本>`（内部证据）就是这条路产出的。两条路的表形状相同，bash 脚本有
全表 diff + cab 抽样解包；PowerShell 脚本只做读回，没有全表 diff。

**第 4 步：安装验收（MSI 路线；EXE 路线的注册语义见 §2.5）**

1. **先卸载旧包**（尤其是 v1.15.1 及更早的 raw MSI）：设置 → 应用 → 已安装的应用，
   或 `msiexec /x <旧包>`。旧包默认在 `C:\Program Files\opi-candidates`；新包默认
   在 `C:\Program Files\opi`。**旧 MSI 与新的 EXE 安装器身份不同（MSI ProductCode
   vs Inno `AppId`），彼此不会自动升级** —— 装 EXE 前先卸掉旧 MSI。旧包不会自动
   清理 `HKLM\SOFTWARE\Microsoft\CTF` 下的注册残留（§6.1 #13），如遇异常先按 #13
   手工清理。
   **Win10 二次安装/覆盖升级必须验证**：装完后把同一个包再装一次（或覆盖升级到
   更高版本），确认安装不回滚、`OpRegisterTsf` 仍成功、输入法不重复也不消失。
   注册代码对已存在 profile 跳过重复写、对机器级默认启用失败做 HKLM 回读容错，
   但这些容错路径没有真机证据；详见 §5.2。
2. **带日志安装**：
   ```powershell
   msiexec /i "dist\opi-<版本>_windows-desktop-processed.msi" /l*v install.log
   ```
   注册 CA 是 Type 1：失败会回滚安装。日志里搜 `OpRegisterTsf`、`WixSilentExec64`、
   `DllRegisterServer`。
3. **注册表核对**（管理员 64 位）：
   ```powershell
   reg query "HKCR\CLSID\{98C51858-590B-404D-8915-21EB44250062}\InprocServer32"
   reg query "HKLM\SOFTWARE\Microsoft\CTF\TIP\{98C51858-590B-404D-8915-21EB44250062}" /s
   ```
   `InprocServer32` 默认值应为 `C:\Program Files\opi\app\tsf_opi.dll`，
   `ThreadingModel=Apartment`；CTF 下应出现语言 profile
   `{A31A137C-1404-42F2-B82C-4E63F15DEFD6}` 和键盘类别。
4. **语言列表**：设置 → 时间和语言 → 语言和区域 → 中文(简体，中国) → 语言选项 →
   键盘 → 添加键盘，应出现「小欧输入法」；或 `Win`+`Space` 轮换。`Get-WinUserLanguageList`
   的 `InputMethodTips` 里应出现 `0804:{CLSID}{PROFILE}`。
5. **失败时先跑诊断脚本**（只读）：
   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\windows\diagnose-tsf.ps1 `
     -InstallDir "C:\Program Files\opi" -InstallLog .\install.log
   ```
   它检查 `HKCR\...\InprocServer32`、`HKLM`/`HKCU` 的 CTF profile 与 `Enable`、
   `Get-WinUserLanguageList` 的 `InputMethodTips` 和安装日志里的
   `OpRegisterTsf`；退出码 0 = 必需项全过，1 = 有失败项。

**账户上下文（静默/企业部署必读）**：

- **交互式安装（推荐）**：MSI 以提权的管理员账户运行，`EnableLanguageProfile` 写的是
  **该账户**的 HKCU；用同一账户登录时立即生效。若安装账户 ≠ 日常账户，目标用户由
  `EnableLanguageProfileByDefault(TRUE)` 写下的 **HKLM 机器级默认启用位**覆盖
  （2026-10-19 新增）。
- **SYSTEM / Intune / SCCM 静默部署**：`EnableLanguageProfile` 写的是 SYSTEM 的
  HKCU，对目标用户无效；这条路径**依赖 HKLM 默认启用位**。已登录的目标用户若仍看不到，
  注销/重新登录，或到 设置 → 语言 手动添加一次键盘。诊断脚本会打印当前账户
  （含是否 SYSTEM）以及 HKLM/HKCU 的 `Enable`。
- 上述 HKLM 默认位在 Win10 上的实际效果**未在真机验证**。需要立即生效时，目标用户
  仍可自己跑一次 `regsvr32 /s "C:\Program Files\opi\app\tsf_opi.dll"`（写该用户 HKCU）。

**图标落点（本轮）**：

- MSI 产品图标（`ARPPRODUCTICON`）与 `opi.exe` 图标：
  `desktop/icons/opi-pet.ico`（实测方法见 §2.5 的「图标」段）。EXE 安装器沿用
  同一份 app image，快捷方式图标也来自它。

> **必须新构建，旧包不会变**：2026-10-19 用户在真机装的三个 MSI
> （本地 `processed-static`、本地 `processed`、CI `opi-desktop-msi`）里的
> **desktop jar 都是旧构建**；本轮改的是源码（TSF 注册、界面），
> 这些包不会因为仓库源码变了而改变。要验证本轮改动，必须按本节重新
> `packageMsi` 并重新后处理；旧包只能用来证明旧行为。

---

## 3. 安装与注册

### 3.1 注册命令

> **2026-10-18 起（2026-10-19 收紧）**：**后处理过的** MSI 会自动执行 `regsvr32 /s`
> （后处理脚本的 `OpRegisterTsf`，序列 5900），**用户不再需要手工注册**。
> ⚠️ Release 里 v1.15.1 及更早的 Windows MSI 是**未后处理**的原始产物（不注册）。
> **【订正 2026-10-20】** 用户入口改为 EXE 安装器：装完由 Inno `[Code]` 自动跑
> 64 位 `regsvr32 /s "{app}\app\tsf_opi.dll"`，退出码只进安装日志、失败**不回滚**
> 安装（§2.5）。后处理 MSI 仍会自动注册（Type 1，失败回滚），但只作内部证据，
> artifact 现名 `opi-msi-internal-v<版本>`。
> 本节保留为核对/回退路径：想确认注册结果、或从旧版单独 DLL 迁移时，仍可按下面的
> 命令手工跑一次。注意 `regsvr32` 写的是**当时 DLL 的全路径**；EXE 与后处理后的 MSI
> 都默认装在 `C:\Program Files\opi\app\tsf_opi.dll`（2026-10-19 起；此前是
> `C:\Program Files\opi-candidates\app\`），手工注册时请指向实际路径。

**【未验证】**（本机无 Windows，下面每一条都没有跑过）。

在**管理员**命令提示符里：

```bat
regsvr32 "C:\path\to\tsf_opi.dll"
```

注销（与注册逆序：先摘 TSF 侧，再删 COM 键，见 `registration::unregister`）：

```bat
regsvr32 /u "C:\path\to\tsf_opi.dll"
```

几条硬约束：

- **必须管理员**。`com_server::DllRegisterServer` 的文档注释写明了理由：
  TSF 侧注册写在 `HKLM\SOFTWARE\Microsoft\CTF` 下；非管理员时 `HKCR` 的写会被静默重定向到 HKCU
  （COM 那半"看似成功"），而 TSF 那几个 API 会失败 —— 于是整体返回失败、`regsvr32` 报错。
  **这是有意的**：宁可响亮地失败，也不要"注册成功但输入法不出现"。
- **用 64 位 `regsvr32`**（`C:\Windows\System32\regsvr32.exe`）。我们的产物是 x86_64；
  在 64 位系统上 `SysWOW64` 下的那个是 32 位的。**【推断】**（平台常识 + 产物三元组，本机无从验证）。
- **DLL 一旦移动就要重新注册**：`InprocServer32` 的默认值写的是注册那一刻的**全路径**
  （`com_server::own_module_path`），路径变了 COM 就找不到它。**【推断】**
- 注册后关闭并重开宿主程序即可让新进程加载新 DLL，**不必注销**；已打开的宿主仍持
  旧映像。**【未验证】**

### 3.2 注册写了什么

分两半，**只有第一半是我们手写的**：

**① COM 侧（手写注册表）** —— 依据在 `registration::register`（2026-10-19 从
`com_server.rs` 拆出，守住单文件 500 行），键名由 `dll::clsid_key` /
`dll::inproc_server_key` 拼：

| 键 | 值 |
|---|---|
| `HKCR\CLSID\{<CLSID_TEXT_SERVICE>}` | 默认值 = `DISPLAY_NAME`（"小欧输入法"） |
| `HKCR\CLSID\{<CLSID_TEXT_SERVICE>}\InprocServer32` | 默认值 = DLL 全路径；`ThreadingModel` = `"Apartment"` |

`CLSID_TEXT_SERVICE` 与 `GUID_PROFILE` 的字面值在 `crates/tsf-opi/src/dll.rs`（**去那里读，别从这里抄**）。
`ThreadingModel` 必须非空 —— 源码注释解释了：缺它时进程内服务器会被 COM 隐式当成 Apartment，而"隐式"正是出问题的来源。

**② TSF 侧（调官方 API，让 TSF 自己写自己的键）** —— `registration::register` 里调用：

- `ITfInputProcessorProfiles::Register`；
- `ITfInputProcessorProfileMgr::RegisterProfile`（Vista+ 的新接口；从同一个
  `CLSID_TF_InputProcessorProfiles` coclass 取），`bEnabledByDefault=TRUE`，
  `LANGID_ZH_CN` = 0x0804 · 中文简体。**注册前先用 `GetProfile` 判存在**：
  已存在就跳过 `Register` + `RegisterProfile`，二次安装/覆盖升级不重写 profile
  （`EnableLanguageProfileByDefault` 仍调用，失败时按下一项回读容错）；
  ⚠️ 它的 `dwFlags` 只收 `TF_RP_*`；`TF_IPPMF_ENABLEPROFILE` 是
  `ActivateProfile` 的标志，**不能**混用；
- `ITfInputProcessorProfiles::EnableLanguageProfileByDefault(..., TRUE)`：写 **HKLM**
  的机器级默认启用位（需管理员）。交互式安装还有下面的 HKCU 位；SYSTEM / Intune /
  GPO 部署没有目标用户的 HKCU，这一位是目标用户之后仍能看到输入法的保险。
  接口文档只列 S_OK / E_FAIL，profile 已存在时重复调用可能返回失败；失败时回读
  `HKLM\SOFTWARE\Microsoft\CTF\TIP\{<CLSID>}\LanguageProfile\0x0804\{<GUID_PROFILE>}\Enable`，
  已是 `1` 则容忍，否则把错误抛出、让安装回滚；
- `ITfInputProcessorProfiles::EnableLanguageProfile(..., TRUE)`：写**当前用户**的启用位
  （`HKCU\Software\Microsoft\CTF\TIP\...\Enable`）。只写 HKLM 的 profile 不够，
  Windows 10 的输入法列表按用户启用状态过滤（2026-10-19 真机「键盘不显示」的
  根因之一）。机器级默认启用已成功后，这一步是 **best-effort**：HKCU 写失败
  不让整个安装回滚（SYSTEM / 异常 HKCU），仍用 `IsEnabledLanguageProfile` 复核；
- `ITfCategoryMgr::RegisterCategory`：`GUID_TFCAT_TIP_KEYBOARD`（"键盘输入法"）+
  Windows 8+ 的 TIP 能力类别 `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT`（UWP/沉浸式宿主）、
  `GUID_TFCAT_TIPCAP_UIELEMENTENABLED`（TIP 自带候选 UI）、
  `GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT`（系统托盘语言栏）。
  **调用前用 `EnumItemsInCategory` 枚举**，已有本 CLSID 就跳过。
  只注册 `TIP_KEYBOARD` 时桌面应用能用、UWP/Store 应用会认为本 TIP 不支持它们 ——
  即「只在桌面可用、其他应用不生效」的注册侧根因。能力标志 `TF_IPP_CAPS_*` 是这些
  类别的读回值（`TF_INPUTPROCESSORPROFILE.dwFlags`），**不是** `RegisterProfile` 的
  `dwFlags`（那里只收 `TF_RP_*`，见上一条）。**【未验证】**：本机只核到 windows crate
  0.62 的常量与签名；注册结果要 Windows 真机读回（§5.2 的诊断脚本已列这四个类别）。

**幂等（2026-10-19 复审改写）**：旧接口 `AddLanguageProfile` / `RemoveLanguageProfile`
已删除 —— 目标 Windows 10 走 Vista+ 的 `RegisterProfile` 正路，两者叠加会在**首次安装**
时就写重复 profile。`TF_E_ALREADY_EXISTS`（0x80040506，保留键冲突）**不是**通用的
「profile 已存在」，不能靠吞它实现幂等；profile 与类别都改为**先查询、存在就跳过**。
`ok_if_already_exists` 现在只为旧接口 `ITfInputProcessorProfiles::Register` 保留，
Type 1 的注册 CA 对其它错误失败并回滚。源码明写这是**有意不手写**的：
「让 TSF 自己写自己的键，就不存在'我把键名/键路径记错了'这一整类失败」。
⇒ **本文不把 TSF 的键路径当实现依据**；§5.2 的诊断脚本只是把注册结果读出来核对。

**容错范围与代价**：跳过已存在的 profile、`EnableLanguageProfileByDefault` 失败时
回读 HKLM `Enable=1` 容错、HKCU 失败不报错 —— 三者都把「重复/异常状态」当正常路径。
代价是可能掩盖两类真实失败：profile 已存在但旧值需要刷新时（如显示名变化）不会更新；
HKCU 写失败是系统性权限问题时安装不会红，只是当前用户少一个启用位。取舍是
「二次安装/覆盖升级不能因重复写而回滚整个安装」优先于「每次刷新所有位」；
对应真机验证见 §2.6 第 4 步与 §5.2。

### 3.3 切到它

**【未验证】**（TSF 的注册行为本机无法验证）。

因为注册用的 `LANGID` 是 zh-CN，预期它会出现在**中文（简体，中国）**这一语言的
键盘列表里（`RegisterProfile` 写 profile，`EnableLanguageProfile` 写当前用户的启用位）：

1. 确认系统已装「中文(简体，中国)」语言。
2. 设置 → 时间和语言 → 语言和区域 → 中文(简体，中国) 的「语言选项」→「键盘」→ 添加键盘 → 应能看到 **小欧输入法**。
3. 用 `Win` + `Space` 轮换输入法。

**【未验证】**：以上三步没有任何人在 Windows 上走过；"出现在哪个列表里""要不要重启"都没有实测。
列表里没有时，先跑 §2.6 第 4 步的 `scripts/windows/diagnose-tsf.ps1`：它会打印
`Get-WinUserLanguageList` 的 `InputMethodTips`、HKLM/HKCU 的 CTF profile 与
`Enable`，以及安装日志里的 `OpRegisterTsf`。

---

## 4. 使用

### 4.1 键位

模式键的判定在 `crates/tsf-opi/src/vk.rs`（**平台中立、本机有单测**），
调用点在 `tsf.rs` 的 `handle_key_inner` —— 都判在引擎**之前**。

| 按键 | 作用 | 符号 |
|---|---|---|
| `Shift` 轻按 | 中 ⇄ 英 | `shift_tap.rs`（判定）+ `tsf_shift.rs`（三拍）→ `TsfLogic::toggle_language` |
| `Ctrl` + `'` | 英文 ⇄ 拼音 | `vk::mode_hotkey` → `ModeHotkey::ToggleEnglish` → `hotkey_target` |
| `Ctrl` + `\` | 符号 ⇄ 拼音 | `vk::mode_hotkey` → `ModeHotkey::ToggleSymbol` |
| `Shift` + `Space` | 全角 ⇄ 半角 | `vk::fullwidth_hotkey` |

**Shift 轻按（2026-10-22 用户报障「无法用 Shift 切换中英文」）**：按下再抬起、
期间没有别的键按下即算一次轻按；`Shift+字母`/方向/Ctrl 组合不是轻按（撤销判定在
`OnTestKeyDown`，覆盖不会进 `OnKeyDown` 的 Ctrl/Alt/Win 组合）。切英文时待处理的
拼音按 W1 **上原文**，⇧ 残留清掉。判定在胶水层而不在 Rust 键路由里 —— Linux 轨的
同一判定在 C++ glue，两轨 Rust 逻辑必须逐事件同构（`two_track_parity`）。
**【未在真机验证】**：TSF 是否把 Shift 的按下/抬起都交给键事件 sink（本改动按
「OnTestKeyUp 答 TRUE 才有 OnKeyUp」实现，见 `tsf.rs` 的 `OnTestKeyUp`）。

- 前两个是**来回切**：已在目标模式就回拼音（`vk::hotkey_target`）；`Shift+Space`
  是同一模式内的**布尔开关**（`logic::toggle_fullwidth` → `Engine::toggle_fullwidth`），
  与那两者分开是源码明写的取舍（`vk.rs` 的 `fullwidth_hotkey` 注释：并进 `ModeHotkey`
  会让 `hotkey_target` 多出一个没有目标模式的假分支）。
- **按住不放的重复事件"认领但不动作"**（`vk::hotkey_should_act`）—— 否则按住期间模式会疯狂来回切。
- 键位是否与 Windows 上的其它软件冲突：**【未验证】**。`vk.rs` 自己写了这一条：
  「这里唯一的依据是'不与 Windows 自身的系统快捷键相撞'这一条常识判断，**没有实测**」。
  （对照：Linux 轨那两个键位是 `strings` + harness **实测**选出来的 —— 两轨键位相同是**约定**，
  不是同一份证据，别把 Linux 那份实测当成这条的背书。）
- ~~⚠️ **模式键会丢弃未提交的缓冲**~~ **【订正 2026-10-28】** 已接线：`tsf.rs::toggle_mode`
  改走 `TsfLogic::switch_mode_confirmed`（拼音/繁体上**原文**、符号模式只清关键字），
  返回值经 `insert_into` 插入当前文档 —— 与 ctl 的 `lang`/`mode`、Shift 轻按同一条收尾。
  **仍未在真机验证**（本机无 Windows）：真机上「打到一半按 Ctrl+'」是否真把原文落进
  文档、宿主是否稳定，未见。

### 4.2 候选操作

| 操作 | 键 | 符号 |
|---|---|---|
| 选词（页内第 1..9 个） | 数字 `1`..`9` | `logic_input_method.rs` 的 `digit_select` |
| 提交首候选 / 提交 | `Space`（缓冲非空）· `Enter` | `handle_space` / `handle_enter` |
| 删缓冲末字 | `Backspace` | `handle_backspace` |
| 上下页 | `PageUp` / `PageDown`；组字中 `-` / `=` | `KEY_PAGE_UP` / `KEY_PAGE_DOWN` 分支；`-`/`=` 见 `handle_printable` 顶部（2026-10-09 补） |
| 切换 ⇧ 大小写（屏上键盘语义） | 程序调用 `TsfLogic::shift_tap` / `shift_long_press` | `logic_shift.rs`（Off → Single → Off，长按 → Lock）；**物理 Shift 不再驱动它**（改走轻按切中英，见 §4.1） |

候选窗里还能**点候选**和**点翻页箭头**（`desktop/.../CandidateWindow.kt` 的
`onSelect` / `onNext` / `onPrev`，接到 `PipeServer.sendSelect` 等）—— 见下一条。

### 4.3 候选点选的回程：**接线了**，但有缺口

**先说结论：这条回程在源码里是接通的，不是 no-op。** 完整调用链（每一跳都能 grep 到）：

```
候选窗点击  CandidateWindow.kt 的 CandidateItem → onSelect
  → Main.kt   onSelect = server::sendSelect
  → PipeServer.sendSelect 发 {"type":"select","index":N}（页内 0 起）
  → candidate_io.rs 读线程 dispatch_line → SharedAction::on_select
  → EngineShared::enqueue_select(index)          ← 只入队，不碰 COM
  → 下一个键事件开头（tsf.rs::handle_key_inner / key_sink.rs::OnTestKeyDown）
    或焦点变化（key_sink.rs::OnSetFocus）→ drain_pending()
  → TsfSharedState::select_and_insert → insert_into → ITfInsertAtSelection
```

翻页两条同理（`SharedAction::on_next_page` / `on_prev_page` → `TsfLogic::next_page` / `prev_page`）。

**但缺口在这里（下面每条都是【推断】，本机无 Windows）：**

1. ~~**候选窗进程没人替你启动。**~~ **【订正 2026-10-20】** 安装器已接线：Inno EXE 的
   HKLM Run（登录自启，卸载删除，§2.5）+ 完成页 `launch-and-restart.cmd` 都会拉起候选窗。
   下面「全仓 `crates/` 搜不到拉起代码」仍然成立，但那是**设计如此**：TSF DLL 只是
   管道客户端，拉起窗口是安装器的职责。（原文：全仓 `crates/` 里搜不到任何拉起
   `desktop/` 进程的代码 —— **【实测】**`grep -rn "CreateProcess\|ShellExecute" crates/`
   零命中；`opi-candidates` 在 `crates/` 里只出现在管道名本身。加上 §2.3 那条
   （`gradlew package` 现在产出的是 Linux `.deb`），
   ⇒ ~~今天在 Windows 上这条回程根本走不到：窗口得用户自己起，而且没有 Windows 版安装包。~~）
2. **点选不是立即插入，要等下一个键事件或一次焦点变化。** 点选只在读线程入队
   （`EngineShared::enqueue_select`），真正的插入在按键线程的 `drain_pending` 里执行
   —— 调用点：`handle_key_inner` / `OnTestKeyDown` 开头，以及
   **`OnSetFocus`（2026-10-28 接线）**。⇒ 点了候选但**既不按键、也不发生焦点变化**
   = 字不出现；点了候选接着打字/切走窗口 = 先插入刚才点的词，再处理新键。
   **别把 `OnSetFocus` 当成点选的即时触发**：候选窗是**非激活**窗口（`Main.kt` 的
   `focusable = false`，`WS_EX_NOACTIVATE` 语义），点它不产生焦点事件，所以
   「点完原地不动」没有任何回调可接。这是源码写明的**有意代价**（`state.rs` 模块头：
   升级路径 = message-only 窗口 + PostMessage，或 GIT 代理），不是 bug ——
   但用户会当成 bug，文档里必须写。**真机未验证**。
3. **逐字确认（2026-10-17）**：整串无词组精确匹配时（前缀扩展不算），点选先「确认一个字」——它留在候选窗
   的缓冲显示里（管线协议 `show.buffer` 现在带「中guo」这类片段，`TsfLogic::display_buffer`），
   最后一段确认才一次提交整串；点中间的字不会立刻插入，这是语义不是缺口（引擎级，
   门禁 `tests/segment_commit.rs`）。
4. ~~**目标文档取自上一次键事件。** 点了候选、还没按键就换文档，字会落进**上一个**文档~~
   **【订正 2026-10-28】** 已修：排队动作带入队时的目标世代（`EngineShared::target_seq`），
   按键线程发现新键事件的 context 与上次不同就推进世代，旧动作在执行点被丢弃
   （`select_and_insert` / `insert_text` 里判）。不会插错文档；代价是那次点选丢了
   —— 旧文档此刻已没有合法插入目标。**真机未验证**（本机无 Windows）。
5. **队列上限 16 条**（`state.rs` 的 `MAX_PENDING_SELECT`），满了丢弃，不阻塞读线程。
6. **翻页不改本地页码**：窗口只发消息，页码唯一真源是 TSF 回发的 `show`
   （`Main.kt` 与 `CandidateWindow.kt` 都写明了）。

### 4.4 属性设置（四个开关）

**配置文件**：`%LOCALAPPDATA%\opi\settings.ini`（与 `dict_path.rs` 的候选词库目录
同一处；键名与 Linux 侧逐字相同，见 `crates/tsf-opi/src/settings.rs`）：

```ini
Learn=True          # 学习（记住选过的词）
ChinesePunct=True   # 中文标点（，。、与成对引号）；数字紧邻的 `.` 例外，恒出 ASCII `.`（2026-10-17 裁决）
Fuzzy=True          # 模糊音（z/zh、n/l…）
Jianpin=True        # 简拼（声母缩写，nh → 你好）
```

**设置窗**：与候选窗是**同一个可执行文件**（`desktop/` 的 Compose 应用）：

```
opi.exe --settings     # 设置窗（开始菜单「opi settings」也是这条）
opi.exe                # 候选窗（无参数；TSF 走命名管道驱动它）
```

设置窗保存后落盘，并经控制面 `{"op":"reload"}` 让 TSF **立即重读**（2026-10-20 起；
与 `Activate` 同一条 `logic.apply_settings`，重读 `settings.ini` 与词库目录）。只有
候选窗/TSF 没在跑时，才退化为「输入法下一次激活」生效。Linux 同一条保存路径走
`ReloadAddonConfig`（见 `docs/install/linux.md` 的设置窗一节）。

⚠️ **2026-10-12 布局改过**（用户裁决「布局和功能参考主流输入法」）：设置窗从单列长滚动
改成**左分类导航 + 右内容 + 底部状态行**（与 Linux 同一份代码，`SettingsWindow.kt`）。
**2026-10-22 起两平台同页集**（常用 / 按键 / 外观 / 状态栏 / 符号与表情 / 词库 / 五笔 /
关于）：Windows 的 `ShiftToggle` 读侧已接通（`crates/tsf-opi/src/settings.rs`，胶水的
Shift 三拍查它）；**外观**页的皮肤由候选窗/状态栏 1s 轮询跟随，候选字号三档由候选窗
`candidateFontScale` 缩放（不再走 fcitx5 classicui）。页面集合仍是纯函数
`settingsTabs`，单测钉住两平台相同。

**自定义短语（2026-10-24，设置窗「高级」页）**：`custom_phrases.json`（中文套）与
`custom_phrases_en.json`（英文套）在 `%LOCALAPPDATA%\opi\`（与 `settings.ini`、词库目录
同处，由 `dict_path.rs` 的数据目录推导；设置窗侧路径在
`desktop/src/main/kotlin/xyz/erik/opi/candidate/CustomPhraseStore.kt`）。两套**完全独立**：
各自开关、各自表、各自文件；schema =
`{"version":1,"enabled":true,"phrases":[{"trigger":"dz","text":"地址"}]}`，宿主文件是
唯一真源（引擎只持内存）。「高级」页并列两个 `CustomPhrasesPage(english = false/true)`：
列表 10 条/页、显示总数、增删改；导入 JSON / TSV / CSV / `字母=短语` / `字母 空白 短语`，
**任何一条非法 ⇒ 整份拒收**；导出 JSON / TSV；读文件上限 1 MiB。上限 **2000 条**、
trigger **≤32 个 ASCII 字母**（`CustomPhraseCodec` 与 `engine-core/src/custom_phrases.rs`
同值）。**开关缺省关（2026-10-25 用户裁决）**：加短语后必须显式开启；每次开关/增删改
都会写文件并走控制面 `reload`，TSF 立即重读，**不需要重启或重新导入**（候选进程不在时
下次 Activate 生效）。

- **装载时机**：TSF 在**启动**（`crates/tsf-opi/src/com_server.rs`）、**Activate**
  （`tsf.rs`，候选进程不在时的兜底）、**控制面 reload**（`ctl.rs` 的 `CtlOp::Reload`，
  设置窗保存时）三处各自读两套文件并推引擎（`custom_phrases::load` / `load_english`
  + `apply_custom_phrases` / `apply_english_custom_phrases`）。fcitx5 只有配置冷路径
  （见 `linux.md` §4.6）。
- **匹配**：中文套只在 `Mode::Pinyin`/`Traditional` 且方案 Full/Nine/Fourteen 时匹配
  （桌面实际只有 Full）；英文套只在 `Mode::English` 匹配；均要求**精确整串**（去 `'`，
  英文大小写不敏感），命中项前插候选。
- **普通英文的缓冲判据（2026-10-24）**：`logic_input_method.rs` 的 `Mode::English`
  分支在 `!english_pro() && !english_custom_phrases_active()` 时逐字直出；**英文短语
  开关开且表非空** ⇒ 字母进缓冲、候选窗出短语，空格提交首候选。表空或开关关 ⇒ 恢复
  直出。携带缓冲（中→英保留的拼音）仍直传。

**新版本提示与应用内下载（2026-10-22 检测；2026-10-31 起不跳浏览器）**：打开设置窗
自动检测一次，发现新版本在右栏顶部显示「发现新版本 vX → 查看更新」横幅（可关闭，点击
切到关于页）；「关于」页常驻当前版本、「检测更新」和「下载全量安装包 / 下载热更新包」。
数据源首选 `https://opi.erik.xyz/version.json`（发版时更新），读不到才回退解析首页
JSON-LD 的 `softwareVersion`。清单 `packages.windows-x64.full/hot` 给出可直下包（https、
可带 size/sha256）；下载在后台 IO 线程，显示进度、可取消，完成后校验 SHA-256 与大小，
可打开文件/文件夹。**只下载+校验，不自动提权替换**；热更新包是 app 目录 zip，关闭小欧后
手工覆盖安装目录的 app 文件夹。没有 `packages` 时退回下载页
`https://opi.erik.xyz/download`。网络失败不打扰（自动检测静默，只有手动检测才显示原因）。
「符号与表情」页那两个按钮打开的是与 Linux 同一份代码的窗（`SymbolsPage.kt` 的
`SymbolWindow`，表情页在 `EmojiPage.kt`）：页面能看能翻，点选上屏（2026-10-20 起）走
控制面 `\\.\pipe\opi-control-v2` → 候选窗进程 broker → TSF 的 `commit`；设置窗进程
不再需要自己持有候选管道。表情页布局与 Android `keyboard/EmojiPanel.kt`
统一：顶栏 `[← 返回] [自有 | Emoji | 颜文字] [搜索框]`；**自有**是独立子页，4 列网格
显示 `shared/pet/stickers/` 的 12 张小欧 PNG，点击 = 复制图片到系统剪贴板（不经过
命名管道、不提交文本），因此**在设置窗里也能用**；Emoji = 9 个 OpenMoji 分组（不再
含自有），颜文字 = `data/raw/kaomoji.tsv` 的 7 组（`copyEmojiData` /
`copyKaomojiData` / `copyPetStickers` 在构建期把数据与 PNG 拷进程序资源，见
`desktop/build.gradle.kts`）。
设置窗另有「表情中显示自有」（`settings.ini` 键 `ShowOwn`，缺省 **False**，仅候选窗/
设置窗读取，TSF 不读）：关闭时顶栏只有 Emoji / 颜文字，打开后才有「自有」子页。
上面那段 `settings.ini` 的四个 TSF 键不受影响。

**「属性」入口已接线**（2026-10-08）：服务对象实现了 `ITfFunction` + `ITfFnConfigure`
（`tsf.rs`），`Show()` 会拉起设置窗（`configure.rs`：从 **DLL 同目录** →
**INSTALLDIR**（DLL 目录的上一级，即 `C:\Program Files\opi`）→
`%LOCALAPPDATA%\opi\` 逐候选找 `opi.exe`，都找不到就**什么都不弹**）。
控制面板那一页取它的办法是 `CoCreateInstance(CLSID, IID_ITfFnConfigure)`（MSDN 的
`ITfFnConfigure` 页面）——⚠️ **本机只验到 `cargo check`**，真机上「属性」按钮会不会出现、
点了起不起得来，都要在 Windows 上看（§6.1 有这一条）。

**2026-10-20 起 Windows 有安装包**（§2.5）：EXE 安装器把 `opi.exe` 放在
`C:\Program Files\opi`，并在开始菜单建「opi settings」入口；「属性」从 DLL 目录的
上一级找到的就是它。设置窗本身是否能在真机拉起仍是【未验证】。

---

## 5. 验证

### 5.1 词库有没有被找到（最容易错的一级）

**坏消息先讲：`dict_path` 成功装载时只 `eprintln!` 一行，而那一行在 GUI 宿主里通常看不见。**
`com_server::CreateInstance` 的注释自己写着：「这一行在 GUI 宿主里通常看不到（无 stderr）；
要真看得见得上 `OutputDebugStringW`（**未实现**）」。
⇒ **别把"没看到日志"当成"没加载"。** 日志的**格式**在 `DictLoad::log_line`（有主机单测），
但**送达**没人验过（**【未验证】**：从 cmd 启动 GUI 程序时 stderr 会不会接上，本机无从验证）。

能做的核对，按可靠性排序：

1. **逐个候选查文件在不在**（在目标机上）：
   ```bat
   echo %OPI_DICT_PATH%
   dir "<DLL 所在目录>\luna.opid"
   dir "%LOCALAPPDATA%\opi\luna.opid"
   ```
   （第二行的尖括号是占位符，换成 DLL 实际所在的目录 —— **不是**批处理的 `%~dp0`，
   那个是脚本自身的目录，与 DLL 无关。）
2. **环境变量这一级最容易出错，三个坑**（都来自 `dict_path.rs`）：
   - 它要的是**文件全路径**，不是目录。写目录名不会被当成目录 + 文件名去拼。
   - **空串等同未设置**（`set OPI_DICT_PATH=` 就是清空）。
   - 它**优先级最高** ⇒ 开发期留下的一个过期 `OPI_DICT_PATH` 会**静默压过**你刚打包进去的那一份。
     排障时先 `echo %OPI_DICT_PATH%`。
3. **行为侧代理**（**【未验证】**，需要 Windows）：
   只有内置回退词库时，同一个拼音的候选会明显少于装了真词库时
   —— 回退词库是 `data/raw/fallback.tsv`（**条数以该文件行数为准**，别抄本文里的数字）。
   打一个常用拼音，看候选是"两三条"还是"一屏"。
4. **本机能验的那一半**（**【实测】**）：
   ```bash
   cargo test -p tsf_opi --no-fail-fast
   ```
   跑的是 `dict_path_tests.rs` 里对**四级顺序与各级失败语义**的断言（平台中立、纯函数）。
   它证明**顺序对**，不证明**目标机上的文件在**。

### 5.2 注册成没成（真机验证清单）

**【未验证】** —— **本机（Linux）没有在真机桌面 Windows 上装过任何会注册的包**；
CI 的 `windows-install-test`（MSI）与 `windows-exe-install-test`（EXE）只做容器冒烟
并抓日志（§2.5），不是真机验证。2026-10-19 用户真机报障（v1.15.1，未后处理包：
有进程、键盘不显示）是本轮修复的**直接证据**；但「注册能不能成功」仍要装一次才知道。
装完后按顺序核：

> ⚠️ **先处理旧包**：如果机器上已经装过 v1.15.1 的**原始** MSI（默认在
> `C:\Program Files\opi-candidates`），请先卸载它再装新包。新 EXE 默认装到
> `C:\Program Files\opi`，且与旧 MSI 的安装器身份不同（Inno `AppId` vs MSI
> ProductCode），**不会自动升级旧 MSI**。卸载旧包不会删除 `%LOCALAPPDATA%\opi`
> 里的用户数据。覆盖安装/二次安装（EXE 与 MSI 各自）都仍无真机证据。

> ⚠️ **Win10 二次安装/覆盖升级必须单独验证**（2026-10-19 复审要求）：在同一台机器上
> 把**同一个包再装一次**（或覆盖升级到更高版本），确认注册仍成功、安装不回滚、
> 语言列表里输入法不重复也不消失。EXE 路线重装会先 `taskkill /f /t /im opi.exe`
> （失败只记日志）；MSI 路线看 `OpRegisterTsf`。注册代码已避免重复写已存在的
> profile、`EnableLanguageProfileByDefault` 失败时回读 HKLM `Enable` 容错、HKCU
> 改为 best-effort，但**这些路径都没有真机证据**（本仓库只在 Linux 上验证）。
> 若二次安装失败，先看安装日志里注册动作前后的错误码。

0. **一次性诊断（推荐先跑）**：装完运行 §2.6 第 4 步的
   `scripts/windows/diagnose-tsf.ps1 -InstallLog .\install.log`（EXE 传入 `/LOG=`
   指定的那份日志；脚本同时识别 MSI 的 `OpRegisterTsf` 与 EXE 的
   `opi: regsvr32` 行）。它只读，退出码 0/1 直接说明必需项是否齐全；下面几项是它的
   手工版本。
1. **COM 注册键**（管理员 cmd / PowerShell）：
   ```bat
   reg query "HKCR\CLSID\{98C51858-590B-404D-8915-21EB44250062}\InprocServer32"
   ```
   默认值应为 `<安装目录>\app\tsf_opi.dll`（后处理后的默认安装目录是
   `C:\Program Files\opi`；若安装时改过目录，以实际为准），`ThreadingModel` 应为
   `Apartment`。CLSID 来自 `crates/tsf-opi/src/dll.rs` 的 `CLSID_TEXT_SERVICE`
   （`clsid_key` 会补花括号）。
2. **TSF 侧注册**（由 `ITfInputProcessorProfiles` / `ITfInputProcessorProfileMgr` /
   `ITfCategoryMgr` 官方 API 写入，源码有意不手写键路径，见 §3.2）：
   ```bat
   reg query "HKLM\SOFTWARE\Microsoft\CTF\TIP\{98C51858-590B-404D-8915-21EB44250062}" /s
   reg query "HKCU\SOFTWARE\Microsoft\CTF\TIP\{98C51858-590B-404D-8915-21EB44250062}" /s
   ```
   应能看到语言 profile（GUID `{A31A137C-1404-42F2-B82C-4E63F15DEFD6}`，
   `GUID_PROFILE`）、`Enable=1`（当前用户）与键盘类别
   （`GUID_TFCAT_TIP_KEYBOARD`）的条目。**只有 HKLM 而没有 HKCU 的 `Enable`**
   是 2026-10-19 之前那版注册的典型状态：profile 存在但用户列表不显示。
3. **`regsvr32` 本身成功**：
   - **EXE 用户入口**：Inno `[Code]` 在文件落盘后静默执行 64 位 `regsvr32 /s`，
     退出码写在安装日志的 `opi: regsvr32 /s tsf_opi.dll exit code <n>` 一行
     （静默安装用 `/LOG=` 指定日志）。**非 0 不回滚安装** —— 键盘不出现时先看这一行，
     需要时手工再跑一次 `regsvr32 "C:\Program Files\opi\app\tsf_opi.dll"`。
   - **MSI 内部证据**：由 `WixSilentExec64` 执行；注册 CA 是 **Type 1（Return=check）**
     ⇒ 失败会让**安装失败并回滚**。排障看 `msiexec /i <包> /l*v install.log` 里
     `OpRegisterTsf` 与 regsvr32 的返回码。
   ⚠️ 旧构建（如 v1.15.1 包内 DLL）动态依赖 `VCRUNTIME140.dll`：没装 VC++ 运行时的
   机器上这一步会以 `0x8007007E` 失败 —— 新 CI 产物已改静态 CRT（§2.5 第 1 条）。
4. **键盘列表出现「小欧输入法」**：§3.3 的第 1–3 步（中文(简体，中国) → 语言选项 →
   键盘），或 `Win`+`Space` 轮换。**这是端到端判据**：前三步都对但列表里没有，说明
   TSF profile 没注册上，回看第 2、3 步。

TSF 那一半是官方 API 写的，**没有"预期键路径"可对照** —— 源码有意如此（见 §3.2）。

### 5.3 能不能打字

> **【订正 2026-10-21 真机】** 已在真实 Windows 10 + Notepad（TSF 宿主）上验证：
> 输入 `nihao` → 候选窗显示「你好」→ 空格上屏；`Ctrl+'` 切英文后 `hello` 直插；
> 再切回中文 `shijie` → `世界`。状态栏在切到 opi 后出现，`中/英` 随模式翻转；
> 控制面 `{"action":"lang"}` 返回 `ok:true`。**这条不再是【未验证】**。
>
> 本轮真机同时钉死了四个「源码看着对、实际不工作」的根因（都已修）：
>
> 1. **`OnTestKeyDown` 只认领热键**：TSF 只在它返回 TRUE 时才调 `OnKeyDown`，
>    旧实现让普通字母全部直落应用 —— 一个字都进不了引擎。现改为「热键 / 普通键
>    → TRUE；Ctrl/Alt 组合与 Win（组合）键 → FALSE」，引擎不消费的键仍由
>    `OnKeyDown` 返回 BOOL(0) 放行。
> 2. **候选进程零窗口即退出**：Compose 1.11 的 `application` 在没有任何 Window 时
>    会 `Recomposer.close/join` 后返回，候选进程启动即退出，两条命名管道都不存在。
>    现始终创建隐藏的候选 Window（`visible` 跟随模型）。
> 3. **同步命名管道的读写互锁**：同一句柄上未完成的阻塞 `ReadFile` 会把 `WriteFile`
>    串行化 —— 按 `n` 出候选后按 `i` 永久冻住宿主 UI。两侧读线程均改为
>    `PeekNamedPipe` + 有数据才 `ReadFile`（20ms 轮询）。
> 4. **`InsertTextAtSelection` 的 `TF_IAS_NOQUERY` 会崩宿主**：Notepad 在
>    `textinputframework.dll` 里 `0xc0000005`。改为 flags=0（查询范围），并把编辑
>    会话引用无条件存进 `keepalive`；`RequestEditSession` + 插入已真机通过。
>
> 历史记录（2026-10-21 之前）：**【未验证】—— 这是整份文档里最重要、也最没底的一条。**

`crates/tsf-opi/src/state.rs` 的模块头写明了真机验收的第一件事：

> 真机验收**第一件事就是点一次候选、再敲一个键**，看文本有没有落进文档、宿主进程是否还在。
> 没插入 → 看 `insert_into` 打出的那行 HRESULT。

它列了 `insert_into` 里三种**静默放弃**（都是"正常情况"，不是错误路径）：
`client_id` 为 0（尚未 `Activate`）、`pic` 转不出 `ITfInsertAtSelection`、
以及 `RequestEditSession` 失败（这一类**会**打一行日志，成因全在宿主进程里）。
放弃后按键仍被吞 —— **用户看到的是"这一下没出字"，不是崩溃。**

---

## 6. 已知限制与未验证项（单列）

### 6.1 本机（Linux）验证不了、且**从未**验过的

| # | 事项 | 落点 |
|---|---|---|
| 1 | 链接：需要 MSVC 导入库 | §1.2 实测卡在 `link.exe` |
| 2 | ~~`tsf_opi.dll` 从未被链接出来过，导出表从未被检视~~ **【已消除 2026-10-09】** 已在 windows runner 上真链接、导出表实测四个 COM 入口（名字未装饰，§2.5）。**【订正 2026-10-18】** DLL 已随 MSI 进入 File 表，安装序列会跑 `regsvr32`；但**真机上的注册结果、宿主加载**仍从未发生 —— 「包里有/表里有」与「能用」是两格 | §1.2、§2.5 |
| 3 | `DllRegisterServer` / `DllUnregisterServer` 的注册效果、管理员判定 | §3.1 |
| 4 | TSF 能否认出这个 TIP、键盘列表里出不出现。**【订正 2026-10-19 复审】** `register()` 已补 `ITfInputProcessorProfileMgr::RegisterProfile`（判存在 + `bEnabledByDefault=TRUE`）、`ITfInputProcessorProfiles::EnableLanguageProfile` 与 `EnableLanguageProfileByDefault`；后处理改用 64 位 `WixSilentExec64`。但仍未在真 Windows 上装过修复后的包，所以「能出现在列表」仍属未验证 | §3.2、§3.3 |
| 5 | 按键回调、composition、文本插入的**全部运行期行为** | §5.3 |
| 6 | 候选窗与插件的命名管道连通（`\\.\pipe\opi-v2`；旧版兼容 `\\.\pipe\opi`） | §4.3 |
| 7 | 四个 `cfg(windows)` 模块**没有任何测试** —— 只有 §1.2 那条 `cargo check` 看过它们 | §1.2 |
| 8 | 候选窗在 Windows 上的~~构建与~~**运行**：**构建已被 §2.5 覆盖**（.msi 真在 windows runner 上打出来了），**跑起来仍然一次都没有** | §2.3、§2.5 |
| 9 | `Ctrl+'` / `Ctrl+\` / `Shift+Space` 在 Windows 上有没有被别的软件占用；**`Shift` 轻按**（2026-10-22）TSF 是否把 Shift 的按下/抬起都交给键事件 sink | §4.1 |
| 10 | 语言栏按钮（`langbar.rs`）的运行期行为：`AddItem` 成不成、`GetIcon` 有没有被调、图标画没画出来 —— 只有 §1.2 那条 `cargo check` 看过它（"挑条目 + 校验 RT_ICON 形态"那半在 `langbar_icon.rs`，有主机单测） | §2.1、`crates/tsf-opi/src/langbar.rs` 头注释 |
| 11 | **「属性设置」的生效时机**：设置窗落盘后要等下一次 `Activate` —— 本机无法验证（`Activate` 由 TSF 调）。读取/容错/落点那半在 `settings.rs`，有主机单测 | §4.4、`crates/tsf-opi/src/settings.rs` |
| 12 | **安装器行为（改安装目录 / 关停运行中实例 / 注册 TSF DLL / 完成页只自启候选窗，2026-10-17 / 10-18 / 10-19 / 10-20）**：表级已改好并可本地复核（§2.5 的后处理脚本 + 读回证据）。**2026-10-19 用户在真机装过的是「未后处理」的 v1.15.1 发布包**（有进程、键盘不显示）—— 这证明了「不跑后处理 = 不注册 = 键盘不出现」。**【订正 2026-10-19 复审】后处理后的三份包**（本地 `processed-static`、本地 `processed`、CI `opi-desktop-msi`）**仍无输入法的根因已定位**：注入的是 32 位 `WixSilentExec`，而随包 `Binary.WixCA` 是 PE32 i386 ⇒ WOW64 调 SysWOW64 的 32 位 `regsvr32`，加载不了 x86-64 DLL。脚本/CI 已改为 `WixSilentExec64` + `WixSilentExec64CmdLine` 并加 PE machine 断言，注册逻辑改为显式判存在 + HKLM 默认启用。**【订正 2026-10-20】EXE 白屏后完成页脚本不再结束/重启 Explorer**：`launch-and-restart.cmd` 只启动 `opi.exe`，`opi.iss` 改用 `runasoriginaluser` 并 `CloseApplications=no`（§2.5）。**【订正 2026-10-23】受控的 shell 刷新回来了**：交互式升级在 `ssPostInstall` 注册新 DLL 后，用原用户令牌 `taskkill /f /im explorer.exe` + 看门狗补启（仅在探测到 Explorer 映射旧 DLL 时；2026-10-23 白屏回归后已回退无条件重启）；首次/静默安装不碰 shell，其它宿主只列日志/完成页、不结束。修复后的 EXE **仍未在真机交互路径装过**（CI 静默安装跳过 `[Run]`），所以 Explorer 重启、`regsvr32` 真注册、ExitDialog 复选框与完成页文案都仍是【未验证】 | §2.5、`scripts/postprocess-windows-msi.sh`、`scripts/windows/opi.iss` |
| 13 | **卸载不清理 TSF 注册（2026-10-19 reviewer m4 记录，有意未做）**：`OpRegisterTsf` 的条件是 `NOT REMOVE` ⇒ 卸载**不执行** `regsvr32 /s /u`；`HKCR\CLSID\{98C51858-590B-404D-8915-21EB44250062}\InprocServer32` 与 `HKLM\SOFTWARE\Microsoft\CTF` 下由 `DllRegisterServer` 写入的条目会**残留并指向已删除的 DLL**。未加卸载动作的原因：`WixSilentExec64` 只有**一个**命令属性 `WixSilentExec64CmdLine`，安装/卸载两条命令会互相覆盖；手写 `RemoveRegistryKey` 又需精确复制官方 API 写入的键，未在真机取证。重装到同一路径会重新覆盖注册；彻底清理 = 卸载前手工 `regsvr32 /s /u <dll>`，已卸载则手工删上述键 | `scripts/postprocess-windows-msi.sh`、§2.5 | 卸载后注册残留；卸载本身的文件删除不受影响 |
| 14 | **Win7 / Win8 / 8.1 不支持（2026-10-19 用户裁决：仅文档标注）**：DLL 的 API set/Rust std 与候选窗的 JDK 21/Compose 下限都在 Win10/11；Win7/8 需另建 legacy 轨道 | §1.1 的说明 | 在 Win7/8 上安装/运行不保证；不在本仓库范围 |
| 15 | **EXE 无重启卸载的锁定 DLL 路径：容器已能模拟，「Explorer 真实持有方式」仍未在真机验证（2026-10-21 更新）**：v1.20.1 真机卸载残留的根因在旧代码：固定目标名 `%TEMP%\opi-tsf-uninstall-<用户>.dll` 让第二次卸载的 `RenameFile` 因目标已存在而失败，跨卷时 `MoveFile` 也失败；失败后 DLL 留在 `{app}`，Inno 报 "Some elements could not be removed"。现改为**同卷 trash 目录**（`{commonappdata}\opi\trash`，装在其他盘时 `<盘符>:\opi-trash`）+ 唯一文件名（时间戳到毫秒）+ 同卷校验；`%TEMP%`/`{win}\Temp` 只在同卷时作回退。成功即删目录，副本 `MoveFileExW(路径, '', MOVEFILE_DELAY_UNTIL_REBOOT)` 登记开机删除；下次安装/卸载先清 trash；全部失败才 `MoveFileExW(原路径, '', MOVEFILE_DELAY_UNTIL_REBOOT)` 并如实记日志（目录保留到重启）。**安装的 `ssInstall`（`[InstallDelete]`/`[Files]` 之前）也先 `UnregisterTsf()` 再 `MoveLockedTsfDll(True)`**：否则 `[Files] restartreplace` 会把新 DLL 排到下次重启（`DoHandleFailedDeleteOrMoveFileTry` 里 `ReplaceOnRestart := True` 后跳过 `MoveFile`），用户装完 1.20.1 仍在跑旧 DLL、`regsvr32` 注册的也是旧 DLL。容器冒烟现在用「改字节 → `LoadLibrary` 映射 → 重装同一包 → SHA-256 对比包内值」验证升级路径，再映射一次验证卸载路径；**容器里的持有者不是 Explorer**，Explorer/TSF 宿主的真实持有方式仍只能真机复验。**升级修好的是磁盘上的文件**：运行中的应用里已加载的旧映像仍是旧代码；仅在 Explorer 映射旧 DLL 时升级会重启 Explorer，其它已打开的宿主必须关闭重开 —— **不需要注销**。另：状态栏锁清理只覆盖运行卸载器的那个用户的 `%TEMP%`（同机其他用户的残留锁文件不处理，无害）。本树没有 Windows 服务，卸载 kill 的就是 `opi.exe` 三种模式 | `scripts/windows/opi.iss`、`.github/workflows/windows.yml`、§2.5 | 真机 Explorer 锁定路径未验证；极端情况下（改名也失败）残留 DLL/目录到重启，交互式卸载会显示 Inno 的 "Some elements could not be removed"，但无重启请求 |

### 6.2 代码里已知的缺口（读源码得出，不是待办猜测）

| # | 缺口 | 依据 | 影响 |
|---|---|---|---|
| 1 | ~~**词库仍没有打包步骤**~~ **【订正 2026-10-20】已修**：EXE 与 MSI 的 app image 都带 `app\luna.opid`（与 DLL 同目录）。**剩余缺口**：TSF 是否真加载了它只在文件层面被 CI 断言，GUI 宿主里看不到装载日志 | §2.4、§2.5 | 文件开箱即命中第二候选；「真加载」仍待真机 |
| 2 | **没有人拉起候选窗进程**；~~候选窗也没有 Windows 安装包~~ **【订正 2026-10-09】** 安装包有了（§2.5 的 `opi-desktop-msi-*` artifact），但它在 Release 之外、90 天过期。**【订正 2026-10-17】** 后处理后的 `.msi` 里「装完点 Finish 自启」已接线（§2.5）—— 但**这条线本身未在真机验证**。**【订正 2026-10-19】** 经 v1.15.1 实物 MSI 核实：到 v1.15.1 为止的 **Release 资产全部是未后处理**的原始包（无注册动作；用户真机报障即此）。**【订正 2026-10-20】** EXE 用户入口已加登录自启（HKLM Run，卸载删除）+ 完成页拉起；未在真机验证 | §4.3 #1、§2.5 | 候选窗现在会自启（真机未验证）；打字链路不依赖它是否已起 |
| 3 | ~~**点选延迟到下一个键事件才插入**~~ **【订正 2026-10-28】** 落地时机加了 `OnSetFocus`（见 #4），但**点了候选原地不动、不敲键**仍不会插入（候选窗非激活，点它无焦点事件）—— 升级路径见 `state.rs` 模块头 | `state.rs` 模块头、`tsf.rs::drain_pending` | 点了不敲键 = 字不出现（仍在，真机未验证） |
| 4 | ~~`OnSetFocus` 空实现（drain 的天然落点没接）~~ **【订正 2026-10-28】** 已接线：`key_sink.rs::OnSetFocus` 调 `drain_pending`（线程依据：`ITfKeyEventSink` 按 tid 注册、TSF 在宿主 UI 线程回调）；`fforeground` 只触发落地、不决定目标 | `key_sink.rs` 的 `OnSetFocus` | 焦点切走/切回时排队点选会落地；不覆盖「点完候选不动」（#3） |
| 5 | ~~点选后换文档 → 插到上一个文档~~ **【订正 2026-10-28】** 已修：`EngineShared::target_seq` 世代号，键事件的 context 变了就推进，旧动作在执行点丢弃（`select_and_insert` / `insert_text`）。代价：那次点选丢了 | `state.rs` 的 `note_target` / `target_seq` | 不再插错文档；真机未验证 |
| 6 | ~~**`Deactivate` 是骨架**~~ **【订正 2026-10-28】** `UnadviseKeyEventSink` 此前已接（与 `Activate` 配对）；本轮补：清队列（`clear_pending`）、释放插入目标与编辑会话保活（`clear_target`）、清引擎缓冲（`logic.clear()`），候选窗由 `sink.disconnect()` 收起。**没有 ITfComposition 可结束**（缓冲只活在候选窗，文档侧无 composition）。**真机未验证** | `tsf.rs::Deactivate`；`state.rs` | 失活后不再残留队列/缓冲；运行时行为待真机 |
| 7 | ~~装载日志走 `eprintln!`，GUI 宿主里通常看不见；`OutputDebugStringW` 未实现~~ **已修（2026-10-28）：`tlog!` 同时写 stderr 与 `OutputDebugStringW`** | `logging.rs` | 排障需 DebugView / 调试器 |
| 8 | ~~繁体（`trad.opid`）在 Windows 上无从装载~~ **已修（2026-10-28）：`dict_path::load_trad` + `EngineShared::with_dicts`，`trad.opid` 随包** | `dict_path.rs`、`logic_with_dicts.rs`、`desktop/build.gradle.kts` | 繁体查 trad 库；缺/坏时回退主库 |
| 9 | ~~模式键**丢弃未提交缓冲**（不先上屏）~~ **【订正 2026-10-28】** 已接线：`tsf.rs::toggle_mode` 改走 `logic.switch_mode_confirmed`，返回值经 `insert_into` 插入文档 | `tsf.rs::toggle_mode`、`logic.rs` | 打到一半切模式不再丢拼音/繁体原文；真机未验证 |
| 10 | ~~候选窗单页显示上限 `MAX_CANDIDATES = 8` 与 Rust `logic::PAGE_SIZE` 同为 8，**但两者之间没有门禁**~~ **【订正 2026-10-28】** 改为**去重**而不是加门禁：`PAGE_SIZE` 只在 `engine-core/src/router.rs` 定义（tsf 轨重导出），桌面 `CandidateWindow.kt` 删除 `MAX_CANDIDATES`，直接渲染 wire `show.candidates`（协议本就只发当前页）—— 不再有第二份上限字面量可漂移 | `engine-core/src/router.rs`、`logic.rs`、`CandidateWindow.kt`、`Main.kt` 协议头 | 桌面按 wire 页长显示；无副本可漂 |
| 11 | ~~**模式热键不认键盘布局**~~ **【订正 2026-10-28】** 已改按**字符**匹配：`key_state::layout_char` 用 `ToUnicodeEx`（Ctrl/Alt 清零、Shift 保留）取当前布局字符，`vk::mode_hotkey(ch, states)` 只认 `'` / `\`。Ctrl+Shift+' 得 `"` ⇒ 不命中（与 fcitx5 轨 keysym 语义一致）。**非 US 布局本机无法实测** | `key_state.rs::layout_char`、`vk.rs::mode_hotkey` | 非 US 布局理论上跟随键帽字符；**未验证** |
| 12 | 语言栏按钮**左键单击打开设置**（2026-10-20 起：`langbar.rs` 的 `OnClick` → `configure::launch_settings()`，与状态栏 LOGO/设置入口一致）；其它点击仍无动作（不假装有功能）。DLL **没有文件图标**（`.rc` / 资源编译器在本机与 CI 都不可验证 ⇒ 走 `include_bytes!`，代价就是缺这一块） | `langbar.rs` 的 `OnClick` 与模块头「为什么不用 .rc」 | 资源管理器里 DLL 显示默认图标 |
| 13 | **`ITfFnConfigure` 只过 `cargo check`**：控制面板里那个「属性」按钮会不会出现、点了拉不拉得起设置窗，本机无法验证；拉起的候选路径判定有主机单测（`settings_exe.rs`），Win32 那一步没有 | §4.4、`crates/tsf-opi/src/configure.rs` | 真机上「属性」可能不出现/不响应 |
| 14 | ~~**设置窗没有 Windows 安装包**~~ **【订正 2026-10-09】** 安装包有了：`desktop/build.gradle.kts` 的 `targetFormats` 加了 `Msi`（原来只列 `Deb`），`.github/workflows/windows.yml` 在 Windows 主机上真打 `.msi`（§2.5）。剩下的缺口：**不在 Release 里、未签名、artifact 90 天过期**，拿到手得自己去 Actions 取 | §4.4、§2.5 | 装完也不会自己出现（本表 #2：没人拉起它） |
| 15 | ~~**W1「切中→英直接确认」在 Windows 侧只有状态机**~~ **【订正 2026-10-28】** 已接线：`tsf.rs::toggle_mode(hot, pic)` 调 `switch_mode_confirmed` 并把返回值插入 `pic` 文档（空 = 推快照刷新窗口，与 Shift 轻按同形）。语义仍为**一律原文**：拼音/繁体上缓冲原文、符号模式的关键字只清不提交 | `logic.rs`、`tsf.rs::toggle_mode`、`tsf_shift.rs` | 与 #9 同一条修复；真机未验证 |
| 16 | **桌面三件套在 Windows 上已接控制面（2026-10-20）**：符号页点选经 `\\.\pipe\opi-control-v2` broker 送 TSF `commit`；状态栏 `--statusbar` 由 **EXE 安装器**的 HKLM64 Run 值随登录自启（卸载时删），jpackage MSI 仍没有这条 Run 值。~~`symbol_categories.tsv` 仍未拷进 MSI/EXE 资源~~ **【订正 2026-10-21】** 已随包：Compose `files` 集合把 `symbol_categories.tsv` 放进 app image 的 `app\`（与 `luna.opid`/DLL 同目录），`symbolDataCandidates` 增加该顺位；windows 工作流断言 app image 与安装目录里都有它 ⇒ 状态栏「符号大全」按钮不再打开空页 | `desktop/build.gradle.kts`、`SymbolCategories.kt`、`.github/workflows/windows.yml`、`scripts/windows/opi.iss` | EXE 装法在切换为 opi 后能看到状态栏（未切换时隐藏）；符号数据开箱可用 |
| 17 | **虚拟键盘 Windows 路径（`SendInput`）只过编译**：本机无 Windows，运行期未验证；一并如实记：Linux 的 XTEST 注入与状态栏窗口**也未做 GUI 冒烟**（本机无 Xvfb，待真桌面时间窗）——逻辑层（键表物理键码、可见性组合、注入序列映射）已测 | `desktop/` 的 `VirtualKeyboard.kt`、`docs/install/linux.md` §4.8 | 未验证 ≠ 不可用，但也别当已验证 |

### 6.3 交付这件事本身

**Windows 用户入口是 Release 里的 `opi-<版本>-windows-x64-setup.exe`**（v1.20.2 起）。
「仓库无安装文档」此前是审计记录在案的共同拦路项之一 —— 本文件是它的第一部分，
仍只覆盖 Windows 一路。

> **【订正 2026-10-18】** Release 从 v1.11.0 起带 `opi-<版本>_windows-desktop.msi`；
> **从 v1.13.0 起 MSI 内含 `tsf_opi.dll`**，不再单独发布 DLL 资产
> （用户裁决「DLL 不要单独发布，用户不会用」）。旧版本仍可在 Release 里找到单独的 DLL，
> 但那是历史产物。「本文档…未经验证」的性质**不变**：产物能编出来 ≠ 装上能用。
>
> **【订正 2026-10-19】** 上一条的「自动注册」**在当时并不成立**：Release 里 v1.15.1
> 及更早的 MSI 是 jpackage 原始产物，**没有**注册动作（发布流程从未跑后处理脚本）。
> 用户在真机装 v1.15.1 后键盘不显示，原因就在这里。即日起 CI 的
> `opi-desktop-msi-v<版本>` artifact 是后处理过的；**Release 资产也必须取这一份**
> （raw 中间物已自动删除；自行拿 jpackage 产物安装会重现「有进程、键盘不显示」）。
>
> **【追加 2026-10-19 晚】** 用户在真机装过的三个 MSI（本地
> `processed-static`、本地 `processed`、CI `opi-desktop-msi`）里的 **desktop jar
> 都是旧构建**，且 `registration::register`（当时还在 `com_server.rs`）都还是修复前的版本。本轮对 TSF 注册
> （`RegisterProfile` 判存在 + `EnableLanguageProfile` + `EnableLanguageProfileByDefault`）、
> 64 位 `WixSilentExec64` 后处理、界面的改动，**不会**出现在这些旧包里。
> 要验证本轮改动，
> 必须按 §2.6 重新 `packageMsi` 并重新后处理；旧包只能证明旧行为。
> `processed-static`（Linux `gcab` 重打 CAB）已撤回，不再是交付物。
>
> **【订正 2026-10-20】** 用户入口从后处理 MSI 改为 **EXE 安装器**
> `opi-windows-exe-v<版本>` → `opi-<版本>-windows-x64-setup.exe`（Inno Setup；TSF 注册
> best-effort、失败不回滚）。后处理 MSI 保留为内部证据，artifact 改名
> `opi-msi-internal-v<版本>`；它与旧 MSI 一样**不应**作为发布资产发给用户。
