<!-- SPDX-FileCopyrightText: 2026 erik.xyz -->
<!-- SPDX-License-Identifier: MIT -->

# Windows 端（TSF）编译 / 打包 / 使用

> ### 先读这一段
>
> **Windows 是三个"有端代码"的平台（Android / Linux / Windows，见 `README.md`）里唯一
> 没人装过、没人跑过的一端。本文是一份操作步骤，不是一份验证过的配方。**
> （另外三个平台 iOS / macOS / 鸿蒙只有草案，更早一步 —— 它们的代码连编译器都没过。）
>
> 每一条都带证据档位：
>
> | 标记 | 含义 |
> |---|---|
> | **【实测】** | 2026-09-28 在 Linux 开发机上真跑过，命令与输出在文内 |
> | **【推断】** | 从源码读出来的结论，给出文件 + 符号；没有运行过 |
> | **【未验证】** | 本机没有 Windows，**没有任何人跑过这一步** |
>
> 本文里**【未验证】**的条目占多数 —— 这是现状，不是本文没写完。判定依据是
> `README.md` 的里程碑 M6c：「COM 服务端按目标平台门控，**且仓库里没有打包步骤**
> （词库开箱仍是内置回退那几十条词），待在 Windows 上验收」。
>
> **别把本文当"照此即可"** —— 本项目的既有教训正是「写完了、读起来像完成、实测发现
> 从未被编译器看过」（fcitx5 的 C++ 7 处 API 误写、TSF 曾有的"一个字都插不进去"）。

---

## 1. 编译前提

### 1.1 目标机器上需要什么

| 项 | 值 | 依据 |
|---|---|---|
| 操作系统 | Windows 10 / 11 x64 | 目标三元组只有 `x86_64-pc-windows-msvc`（下面 1.2 有实测）；**Windows on ARM 需要另加 `aarch64-pc-windows-msvc`，本仓库没有为它做过任何事** |
| C++ 构建工具 | Visual Studio 2017+ 或 Build Tools，勾选「使用 C++ 的桌面开发」 | 提供 `link.exe` 与 MSVC 导入库。`crates/tsf-opi/Cargo.toml` 声明的 `windows` crate 依赖在链接期需要导入库 |
| Rust | `rustup target add x86_64-pc-windows-msvc` | 目标不是默认安装的 |
| 词库文件 | `luna.opid`，**可选但强烈建议** —— 不给就只有内置回退词库（见 §2.2） | `crates/tsf-opi/src/dict_path.rs` |
| 候选窗运行时 | JVM（只有要显示候选窗时才需要） | `desktop/` 是 Compose Desktop / JVM 工程 |

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
| `luna.opid` | 见 §2.2 | 词库。可选；缺了就退回内置词库 |
| 候选窗（JVM 应用） | `desktop/` | **独立进程**，见 §2.3 |

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

候选窗是**另一个进程**：Compose Desktop / JVM 应用，经命名管道 `\\.\pipe\opi-candidates`
与插件通信（协议镜像两份：`crates/tsf-opi/src/candidate_io.rs` 与
`desktop/src/main/kotlin/xyz/erik/opi/candidate/Main.kt` 的模块头注释）。

- **不开它也能打字**：`CandidateClient::connect` 连不上时静默退避重试、本次发送放弃，
  降级为 no-op（**【推断】**，见 `candidate_io.rs` 的 `connect` / `send_json`）。
  用户看到的是"没有候选窗"，不是"输入法坏了"。
- 打包格式**当前只声明了 Deb**：`desktop/build.gradle.kts` 的
  `nativeDistributions { targetFormats(TargetFormat.Deb) }`，同一处的注释写着
  「Windows .msi 为验收阶段（Windows 主机上构建）」。
  ⇒ **要在 Windows 上出 `.msi` / `.exe`，得先改这个 `targetFormats`，且必须在 Windows 主机上构建**
  （jpackage / Compose Desktop 不能从 Linux 交叉打包 Windows 安装包）。**【未验证】**
- 不改打包、只想跑起来：`desktop/` 是自带 wrapper 的 Gradle 工程，
  Windows 上用 `gradlew.bat`（`desktop/gradlew.bat` 存在，**【实测】**文件在）。
  `build.gradle.kts` 里设了 `mainClass = "xyz.erik.opi.candidate.MainKt"`，
  故 `.\gradlew.bat run` 应当能直接起窗。**【未验证】**（本机只有 Linux，没有在 Windows 上跑过）。

### 2.4 现状：**没有打包步骤**

这一条必须原样说，别写成已完成：

> **仓库里没有任何打包步骤会把 `luna.opid` 和 `tsf_opi.dll` 放到一起。** 词库通路是通的
> （`dict_path.rs` 的四个候选），但**没有人往那四个位置送货**。装到 Windows 上开箱
> **仍然是内置回退词库**，除非用户自己设 `OPI_DICT_PATH`。

依据（三处，都能现场复核）：

1. `crates/tsf-opi/src/dict_path.rs` 的模块文档自己写着：「本仓库目前**没有** Windows 打包脚本，
   故没有任何构建期拷贝动作 —— 这两个候选就是全部通路」。
2. `crates/tsf-opi/` 下**没有 `build.rs`**（**【实测】**目录清单里不存在该文件）。
3. `README.md` 的 M6c 一行与「词库分发通路」一条都写着 Windows 侧仍无打包步骤。

⇒ 本文的 §3、§4 里凡是涉及 `luna.opid` 的步骤，**都是手工动作**。

---

## 3. 安装与注册

### 3.1 注册命令

**【未验证】**（本机无 Windows，下面每一条都没有跑过）。

在**管理员**命令提示符里：

```bat
regsvr32 "C:\path\to\tsf_opi.dll"
```

注销（与注册逆序：先摘 TSF 侧，再删 COM 键，见 `com_server::unregister`）：

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
- 注册后建议重启一次宿主程序 / 重新登录，让 TSF 重新枚举输入法。**【未验证】**

### 3.2 注册写了什么

分两半，**只有第一半是我们手写的**：

**① COM 侧（手写注册表）** —— 依据在 `com_server::register`，键名由 `dll::clsid_key` /
`dll::inproc_server_key` 拼：

| 键 | 值 |
|---|---|
| `HKCR\CLSID\{<CLSID_TEXT_SERVICE>}` | 默认值 = `DISPLAY_NAME`（"OPI 拼音输入法"） |
| `HKCR\CLSID\{<CLSID_TEXT_SERVICE>}\InprocServer32` | 默认值 = DLL 全路径；`ThreadingModel` = `"Apartment"` |

`CLSID_TEXT_SERVICE` 与 `GUID_PROFILE` 的字面值在 `crates/tsf-opi/src/dll.rs`（**去那里读，别从这里抄**）。
`ThreadingModel` 必须非空 —— 源码注释解释了：缺它时进程内服务器会被 COM 隐式当成 Apartment，而"隐式"正是出问题的来源。

**② TSF 侧（调官方 API，让 TSF 自己写自己的键）** —— `com_server::register` 里调用：

- `ITfInputProcessorProfiles::Register` + `AddLanguageProfile`（`LANGID_ZH_CN` = 0x0804 · 中文简体）
- `ITfCategoryMgr::RegisterCategory`，类别 = `GUID_TFCAT_TIP_KEYBOARD`（"键盘输入法"）

源码明写这是**有意不手写**的：「让 TSF 自己写自己的键，就不存在'我把键名/键路径记错了'
这一整类失败」。⇒ **本文不列 TSF 那几个键的路径，因为源码有意不依赖它们。**

### 3.3 切到它

**【未验证】**（TSF 的注册行为本机无法验证）。

因为 `AddLanguageProfile` 用的 `LANGID` 是 zh-CN，预期它会出现在**中文（简体，中国）**
这一语言的键盘列表里：

1. 确认系统已装「中文(简体，中国)」语言。
2. 设置 → 时间和语言 → 语言和区域 → 中文(简体，中国) 的「语言选项」→「键盘」→ 添加键盘 → 应能看到 **OPI 拼音输入法**。
3. 用 `Win` + `Space` 轮换输入法。

**【未验证】**：以上三步没有任何人在 Windows 上走过；"出现在哪个列表里""要不要重启"都没有实测。

---

## 4. 使用

### 4.1 键位

模式键的判定在 `crates/tsf-opi/src/vk.rs`（**平台中立、本机有单测**），
调用点在 `tsf.rs` 的 `handle_key_inner` —— 都判在引擎**之前**。

| 按键 | 作用 | 符号 |
|---|---|---|
| `Ctrl` + `'` | 英文 ⇄ 拼音 | `vk::mode_hotkey` → `ModeHotkey::ToggleEnglish` → `hotkey_target` |
| `Ctrl` + `\` | 符号 ⇄ 拼音 | `vk::mode_hotkey` → `ModeHotkey::ToggleSymbol` |
| `Shift` + `Space` | 全角 ⇄ 半角 | `vk::fullwidth_hotkey` |

- 前两个是**来回切**：已在目标模式就回拼音（`vk::hotkey_target`）；`Shift+Space`
  是同一模式内的**布尔开关**（`logic::toggle_fullwidth` → `Engine::toggle_fullwidth`），
  与那两者分开是源码明写的取舍（`vk.rs` 的 `fullwidth_hotkey` 注释：并进 `ModeHotkey`
  会让 `hotkey_target` 多出一个没有目标模式的假分支）。
- **按住不放的重复事件"认领但不动作"**（`vk::hotkey_should_act`）—— 否则按住期间模式会疯狂来回切。
- 键位是否与 Windows 上的其它软件冲突：**【未验证】**。`vk.rs` 自己写了这一条：
  「这里唯一的依据是'不与 Windows 自身的系统快捷键相撞'这一条常识判断，**没有实测**」。
  （对照：Linux 轨那两个键位是 `strings` + harness **实测**选出来的 —— 两轨键位相同是**约定**，
  不是同一份证据，别把 Linux 那份实测当成这条的背书。）
- ⚠️ **模式键会丢弃未提交的缓冲**：`tsf.rs::toggle_mode` → `TsfLogic::switch_mode` →
  `Engine::switch_mode`，那条路**只清缓冲、不上屏** —— 打到一半、甚至已经有候选的拼音
  都会一起丢掉。`engine-core/src/engine.rs` 的 `toggle_symbol` 文档把这点写得很明白
  （它那条"先提交待上屏文本"的收尾是给有插入通道的端用的，桌面两轨都没接）。
  **【推断】**，未在真机确认用户手感。

### 4.2 候选操作

| 操作 | 键 | 符号 |
|---|---|---|
| 选词（页内第 1..9 个） | 数字 `1`..`9` | `logic_input_method.rs` 的 `digit_select` |
| 提交首候选 / 提交 | `Space`（缓冲非空）· `Enter` | `handle_space` / `handle_enter` |
| 删缓冲末字 | `Backspace` | `handle_backspace` |
| 上下页 | `PageUp` / `PageDown` | `KEY_PAGE_UP` / `KEY_PAGE_DOWN` 分支 |
| 切换 ⇧ 大小写 | `Shift` | `handle_shift`（Off → Single → Off，长按 → Lock） |

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
  → 下一个键事件开头 tsf.rs::handle_key_inner → drain_pending()
  → TsfSharedState::select_and_insert → insert_into → ITfInsertAtSelection
```

翻页两条同理（`SharedAction::on_next_page` / `on_prev_page` → `TsfLogic::next_page` / `prev_page`）。

**但缺口在这里（下面每条都是【推断】，本机无 Windows）：**

1. **候选窗进程没人替你启动。** 全仓 `crates/` 里搜不到任何拉起 `desktop/` 进程的代码
   （**【实测】**`grep -rn "CreateProcess\|ShellExecute" crates/` 零命中；
   `opi-candidates` 在 `crates/` 里只出现在管道名本身）。加上 §2.3 那条
   （`gradlew package` 现在产出的是 Linux `.deb`），
   ⇒ **今天在 Windows 上这条回程根本走不到**：窗口得用户自己起，而且没有 Windows 版安装包。
2. **点选不是立即插入，要等下一个键事件。** 点选只在读线程入队
   （`EngineShared::enqueue_select`），真正的插入在按键线程的 `drain_pending` 里执行
   —— 而 `drain_pending` 只在 `handle_key_inner` 开头被调用一次。
   ⇒ **点了候选但不再按键 = 字不出现**；点了候选接着打字 = 先插入刚才点的词，再处理新键。
   这是源码写明的**有意代价**（`state.rs` 模块头：「已知代价：插入延迟到下一次按键，
   换来零跨线程 COM 调用」），不是 bug —— 但用户会当成 bug，文档里必须写。
   顺带：`tsf.rs` 的 `OnSetFocus` 是空实现（`Ok(())`），**是 drain 的天然落点，没接**。
3. **目标文档取自上一次键事件。** 点了候选、还没按键就换文档，字会落进**上一个**文档
   （`tsf.rs::drain_pending` 的注释自己记了这条"已接受的边缘"）。
4. **队列上限 16 条**（`state.rs` 的 `MAX_PENDING_SELECT`），满了丢弃，不阻塞读线程。
5. **翻页不改本地页码**：窗口只发消息，页码唯一真源是 TSF 回发的 `show`
   （`Main.kt` 与 `CandidateWindow.kt` 都写明了）。

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

### 5.2 注册成没成

**【未验证】**（本机无 Windows）。可查的点：

```bat
reg query "HKCR\CLSID\{<CLSID_TEXT_SERVICE>}\InprocServer32"
```

`<CLSID_TEXT_SERVICE>` 的值去 `crates/tsf-opi/src/dll.rs` 取（`clsid_key` 会给它补花括号）。
默认值应等于 DLL 全路径、`ThreadingModel` 应为 `Apartment`。

TSF 那一半是官方 API 写的，**没有"预期键路径"可对照** —— 源码有意如此（见 §3.2）。
可观察的替代：§3.3 的键盘列表里出没出现。

### 5.3 能不能打字

**【未验证】—— 这是整份文档里最重要、也最没底的一条。**

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
| 2 | `tsf_opi.dll` 从未被链接出来过，**导出表从未被检视** | §1.2 |
| 3 | `DllRegisterServer` / `DllUnregisterServer` 的注册效果、管理员判定 | §3.1 |
| 4 | TSF 能否认出这个 TIP、键盘列表里出不出现 | §3.3 |
| 5 | 按键回调、composition、文本插入的**全部运行期行为** | §5.3 |
| 6 | 候选窗与插件的命名管道连通（含 `\\.\pipe\opi-candidates` 单实例语义） | §4.3 |
| 7 | 四个 `cfg(windows)` 模块**没有任何测试** —— 只有 §1.2 那条 `cargo check` 看过它们 | §1.2 |
| 8 | 候选窗在 Windows 上的构建与运行 | §2.3 |
| 9 | `Ctrl+'` / `Ctrl+\` / `Shift+Space` 在 Windows 上有没有被别的软件占用 | §4.1 |

### 6.2 代码里已知的缺口（读源码得出，不是待办猜测）

| # | 缺口 | 依据 | 影响 |
|---|---|---|---|
| 1 | **没有打包步骤**：`luna.opid` 与 DLL 不会被放到一起 | §2.4 | 开箱只有内置回退词库 |
| 2 | **没有人拉起候选窗进程**；候选窗也没有 Windows 安装包 | §4.3 #1 | 候选窗看不到（打字仍可用） |
| 3 | **点选延迟到下一个键事件才插入** | `state.rs` 模块头、`tsf.rs::drain_pending` | 点了不敲键 = 字不出现 |
| 4 | `OnSetFocus` 空实现（drain 的天然落点没接） | `tsf.rs` 的 `OnSetFocus` | 同上，放大了 #3 |
| 5 | 点选后换文档 → 插到上一个文档 | `tsf.rs::drain_pending` 注释 | 边缘场景 |
| 6 | **`Deactivate` 是骨架**：没有 `UnadviseKeyEventSink`，没有释放 composition / 候选窗 | `tsf.rs::Deactivate`；`docs/superpowers/specs/2026-08-14-opi-multi-platform-design.md` 的「tsf-opi Deactivate 为骨架（正式范围降级）」一行，明示为"验收补全点" | 待真机验收补 |
| 7 | 装载日志走 `eprintln!`，GUI 宿主里通常看不见；`OutputDebugStringW` **未实现** | `com_server::CreateInstance` 注释 | 排障全靠间接观察 |
| 8 | **繁体（`trad.opid`）在 Windows 上无从装载**：`dict_path::candidates` 里没有它，且 `TsfLogic` 只持一个词库 | `dict_path.rs` 模块头的「已知缺口」 | 繁体模式打不开（与 README「Windows 没有繁体入口」一致） |
| 9 | 模式键**丢弃未提交缓冲**（不先上屏） | `tsf.rs::toggle_mode` → `Engine::switch_mode`；`engine.rs::toggle_symbol` 注释 | 打到一半切模式 = 丢字 |
| 10 | 候选窗单页显示上限 `MAX_CANDIDATES = 8` 与 Rust `logic::PAGE_SIZE` 同为 8，**但两者之间没有门禁**（Rust 那份有 `const` 断言绑 `engine-core`，Kotlin 那份只有一句注释说"一致"） | `CandidateWindow.kt` vs `logic.rs` | 若 `PAGE_SIZE` 改了，窗口会静默少显示最后一个候选 |
| 11 | **模式热键不认键盘布局**：`vk::mode_hotkey` 比的是**裸 VK**（0xDE / 0xDC），即 US 布局上标着 `'` 与 `\` 的那两个物理键位；而字符键走的是布局感知的 `ToUnicodeEx`（`tsf.rs::to_engine_keycode`）。⇒ 非 US 布局（AZERTY / Dvorak）上，要按的是**那个位置**的键，不是键帽上的字符 | `vk.rs::mode_hotkey` 的 `VK_OEM_5` / `VK_OEM_7`；`tsf.rs::to_engine_keycode` | 非 US 布局用户会"按了没反应"（**未验证**：本机没有任何非 US 布局可测） |

### 6.3 交付这件事本身

**Windows 端目前没有可下载产物**（Release 不带 Windows 包），且**本文档是这一端的第一份安装文档**。
「仓库无安装文档」此前是审计记录在案的共同拦路项之一 —— 本文件是它的第一部分，
但**只覆盖 Windows 一路，且未经验证**。
