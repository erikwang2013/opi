<!-- SPDX-FileCopyrightText: 2026 erik.xyz -->
<!-- SPDX-License-Identifier: MIT -->

# Windows 端（TSF）编译 / 打包 / 使用

> ### 先读这一段
>
> **Windows 是三个"有端代码"的平台（Android / Linux / Windows，见 `README.md`）里唯一
> 没人装过、没人跑过的一端。本文是一份操作步骤，不是一份验证过的配方。**
> （另外三个平台 iOS / macOS / 鸿蒙只有草案，更早一步 —— 它们的代码连编译器都没过。）
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

> **Win7 / Win8 / 8.1 不支持（2026-10-19 用户裁决：仅文档标注）**：当前
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
| `luna.opid` | 见 §2.2 | 词库。可选；缺了就退回内置词库 |
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
首启种子一次（之后照常启停，删过不复活）。生效时机 = **下次激活**（TSF 在引擎装载时读该目录）。

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

- **不开它也能打字**：`CandidateClient::connect` 连不上时**单次尝试、本次发送放弃、不睡眠**
  （2026-10-18 性能审计：旧实现在按键线程上 4×50ms 退避，候选窗未启动时每键白卡 200ms；
  现在下一条 show 自然再试，窗口状态每次全量重发），降级为 no-op（**【推断】**，
  见 `candidate_io.rs` 的 `connect` / `send_json`）。用户看到的是"没有候选窗"，
  不是"输入法坏了"。`WriteFile` 仍无超时（候选窗假死时理论上可阻塞按键线程，
  异步写线程是后续项）。
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

> **【补充 2026-10-09 / 订正 2026-10-18】** §2.5 的自动打包**没有**改变这一段：MSI 现在
> 带 `tsf_opi.dll`，但**仍不含** `luna.opid`，也不会把词库放到 DLL 旁边（工作流里没有
> 任何词库拷贝动作）。词库通路照旧走 §2.2 的四级查找 + 手工送货。

### 2.5 自动打包（GitHub Actions，2026-10-09 起）

`.github/workflows/windows.yml` —— 构建 job 在 windows runner 上跑，后处理 job
（`msi-postprocess`，2026-10-19 起）在 `ubuntu-latest` 上用 msitools 跑，
容器安装冒烟 job（`windows-install-test`，2026-10-20 新增）在 `windows-latest` 上跑。
**订正 2026-10-19**：不再写「`ci.yml` 一个字没动」—— `ci.yml` 已有独立的
`packaging-scripts` job（2026-10 起，跑 Linux 安装脚本回归），但它不碰 Windows 构建；
`packages.yml` 未动。它覆盖 §1.2 那张表里本机打 ❌ 的几格：
**DLL 真链接（静态 CRT）、导出/导入表检视、`.msi` 产出、表级后处理（注册动作/安装目录）、
容器内 `msiexec` 冒烟日志**。后处理 job 只做表级注入；`windows-install-test` 只在
Windows Server Core 容器里安装并抓日志，**不**等于真机验证 —— 容器通常缺语言栏/TSF，
`OpRegisterTsf` 失败/回滚会如实留在日志里。「表里注入了」与「Windows 端能用」仍是
两件事（§6）。

| artifact | 内容 | 实测体积（来源 / 单位 / 日期） |
|---|---|---|
| `opi-tsf-dll-v<版本>` | `tsf_opi.dll`（zip 里就这一个文件；**仅供 desktop-msi job 下载打包，不再作为用户入口发布**） | DLL 本体 778,240 B、上传 zip 331,390 B（2026-10-09 run，旧构建） |
| `opi-desktop-msi-raw-v<版本>` | jpackage **原始** `.msi`（zip 里一个文件）。**仅内部中间物**（YAML 注释标 internal）：没有注册动作，只供 `msi-postprocess` 下载；后处理成功后由该 job 删除，**正常 run 的最终 artifacts 里不应出现** | 上传的 **zip** 58,683,236 B（2026-10-09 run）；**未压缩 MSI** 62,034,236 B（2026-10-19 本机核对的 v1.15.1 发布资产）。两者不是同一次构建，别混比 |
| `opi-desktop-msi-v<版本>` | **后处理后的** `opi-candidates-<版本>.msi`（zip 里一个文件）。**这才是用户入口**（`msi-postprocess` job 产出） | **未压缩 MSI** 62,443,008 B（2026-10-19 本机由上一行的 v1.15.1 raw 处理得到；+408,772 B）。CI 上传的是 zip，别再拿这个数当 zip 体积 |
| `opi-install-logs-v<版本>` | `windows-install-test` 在 Windows Server Core 容器里跑 `msiexec` 的日志与目录/注册表输出（`install-custom.log` / `install-default.log` / `dir-*.txt` / `reg-*.txt`）。容器缺语言栏/TSF，失败/回滚会如实留在日志里 | 每次 run 新生成 |

名字里的版本来自 `Cargo.toml` 的 `[workspace.package] version`（本仓六 crate 共用的单一来源），
并与 `desktop/build.gradle.kts` 的 `opiAppVersion`（= jpackage 的 `packageVersion`）**比对**：
两处不等时工作流直接红 —— 不打出「名字与内容各说各话」的包。

**怎么拿**（Actions artifact 默认保留 90 天；**不自动附 Release**，与 `packages.yml`
「发版产物归人管」同一条约定）：

```bash
gh run list   --workflow=windows.yml                 # 找最近一次成功的 run
gh run download <run-id> -n opi-tsf-dll-v<版本>      # 或 -n opi-desktop-msi-v<版本>
```

⚠️ **正常 run 的最终 artifacts 里不应出现 `-raw-`**：raw 只是
`desktop-msi → msi-postprocess` 的 job 间中间物，后处理成功后自动删除。若还能看到它，
说明后处理 job 失败（raw 保留用于排障，也不要安装）。用户唯一入口是
`opi-desktop-msi-v<版本>`。

或在 Actions 页面（仓库 → Actions → Windows 打包）点进某次 run 下载。
**触发**：手动 dispatch，或 push 到默认分支且改动落在 `crates/tsf-opi/**` / `desktop/**` /
`scripts/postprocess-windows-msi.sh` / `.github/workflows/windows.yml`。⚠️ 本仓约定 workflow
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
- **`launch-and-restart.cmd`**：完成页复选框拉起的脚本（启动候选窗 + 重启 Explorer）；
- **PUA 字体 `opi-emoji.ttf` + `install-emoji-font.ps1`**（2026-10-19 加）：字体真源是
  `shared/pet/emoji-font/opi-emoji.ttf`，`desktop/build.gradle.kts` 在
  `OPI_TSF_DLL` 分支把它作为**普通文件**挂进 jpackage（同时仍以
  `pet-font/opi-emoji.ttf` 进 jar classpath）。MSI 里的落点是
  `INSTALLDIR\app\opi-emoji.ttf`；完成页脚本以登录用户身份把它复制到
  `%LOCALAPPDATA%\Microsoft\Windows\Fonts\opi-emoji.ttf`，写
  `HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts` 的
  `Opi Emoji (TrueType)`，并广播 `WM_FONTCHANGE`。⚠️ 只有勾选 Finish 才执行；
  `msiexec /qn` 不装字体。字体含 **CBDT/CBLC + SVG** 两套彩色表、**没有
  COLR/CPAL** ⇒ Windows 10 原生 DirectWrite 宿主是否渲染未经真机验证；应用自身
  走 Compose/Skia 的 classpath 字体，不依赖系统安装；
- **没有** `luna.opid`，也不设置词库环境变量（§2.4 的判断不变）。

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
   `...|opi-candidates` 改为 `opi` ⇒ 默认装到 `C:\Program Files\opi`。只改这一行，
   其余行原样整表导回；`packageName` 不动（TSF 的 `settings_exe` 依赖
   `opi-candidates.exe`）。
2. 执行序列 797 加一条 `"taskkill" /f /t /im opi-candidates.exe`（引号是 WixQuietExec 的
   硬要求；排在 `RemoveExistingProducts@798` **之前** —— 否则运行中的旧实例会走
   「改名+重启后删」那条路，装完还活着）。
3. 执行序列 5900（InstallFiles 之后、InstallFinalize 之前）经 `WixCA` 的
   **`WixSilentExec64`**（**Type 1，Return=check**；不是 65）执行
   `"regsvr32" /s "[INSTALLDIR]app\tsf_opi.dll"`，注册随 MSI 安装的 TSF COM 服务器。
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
   拉起 `launch-and-restart.cmd`：以**登录用户**身份启动候选窗，然后
   `taskkill /f /im explorer.exe`。**只杀不 `start`**：从提权的 msiexec 里
   `start explorer.exe` 会得到管理员令牌的 shell（UAC 旁路）；shell 由 Winlogon 的
   `AutoRestartShell`（默认 1）以**用户自己的完整性级别**拉回。

复选框文案因此变成 `Launch opi-candidates and restart Explorer`，完成页还有一行说明
（`WIXUI_EXITDIALOGOPTIONALTEXT`）：Explorer 会重启以重新加载输入法，其它已打开的应用
要各自重启才换到新版本。脚本自带前置断言（槽位空、基线行在）、**显式后处理标记**
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
需要真机验证请用 CI 的 `opi-desktop-msi-v<版本>`（静态 CRT、含本轮修复）。

**本地静态 CRT CAB 重打包：已撤回，不是交付物（2026-10-19 晚）**。曾用 Linux
`gcab` 重打 `Data.cab`、替换 File 键
`filedfd5da438ec03d8391aeff2429bdad72` 并重跑后处理，产出过
`dist/opi-1.15.1_windows-desktop-processed-static.msi`。它只做过 `msiextract` /
表级读回（195 个文件、DLL 哈希与导入表、四个导出、幂等），**从未被 `msiexec`
实际安装过**；Linux `gcab` 产出的 cab 不满足「可交付」的证据档位，故撤回该文件与
把它当测试包的用法。需要静态 CRT 的真机包，只能从 CI 的 `opi-desktop-msi-v<版本>`
或 §2.6 的 Windows 本机构建取得。

**本地 `processed` 包只作表级证据**：`dist/opi-1.15.1_windows-desktop-processed.msi`
（若本机还保留）是用修订后脚本处理 v1.15.1 raw 得到的，用来复核 `INSTALLDIR=opi`、
Type 1 的 `OpRegisterTsf@5900`、`WixSilentExec64` 与 `OPI_POSTPROCESSED` 标记。它的
**desktop jar 与 DLL 仍是旧构建**，不含 2026-10-19 之后的源码改动，也**没有**本轮新增的
注册修复（`RegisterProfile` 判存在、`EnableLanguageProfileByDefault`、删除旧
`AddLanguageProfile`）与新增的 `opi-emoji.ttf` / `install-emoji-font.ps1` ——
**不要拿它验收界面、输入法列表或字体，也不要发布**。需要新界面/新注册逻辑时，
必须重新构建（§2.6）。

**升级后哪些进程会重载、哪些不会**（全部【未验证】：本机没有 Windows）：

| 对象 | 会发生什么 |
|---|---|
| 新启动的进程 | 加载磁盘上的新 DLL（MSI 的 File 表已含 `tsf_opi.dll`） |
| Explorer / shell / 语言栏 | 勾选复选框点 Finish → 脚本结束 explorer → Winlogon 自动重启 → 重新加载 |
| 已打开的其它应用（Office / 浏览器 / 编辑器…） | **仍持旧 DLL**，直到各自关闭重开 |
| 候选窗进程 | 装前被 797 结束、装后由完成页脚本拉起 |
| COM 注册 | `OpRegisterTsf@5900` 经 `WixSilentExec64` 调 64 位 `regsvr32 /s` 注册 per-machine（Type 1：失败则安装失败/回滚）；卸载**不清注册**（`NOT REMOVE` ⇒ 不跑 `regsvr32 /u`，`HKCR`/`HKLM\SOFTWARE\Microsoft\CTF` 残留指向已删 DLL 的条目，见 §6.1 #13） |

- **静默 / 基本 UI 安装**（`msiexec /qn`、`/qb`）不出现 ExitDialog ⇒ **不会重启 Explorer**。
  需要手动：`taskkill /f /im explorer.exe`（等 Winlogon 拉回），或注销/重启。
- **用户取消勾选** ⇒ 自启与 Explorer 重启都不发生（两个动作共用这一个复选框）。
- **失败回退**：注册（`OpRegisterTsf@5900`，Type 1）**失败会让安装失败并回滚**（有意：
  宁可响亮失败，也不留「装成功、键盘不出现」的半注册状态）。完成页的启动/重启 Explorer
  （`OpLaunchApp`，完成页 Finish 时触发、在 InstallFinalize 之后）失败则不影响安装结果。
  若 `AutoRestartShell=0`（非默认值），shell 不会自动回来 —— 任务管理器 → 文件 →
  运行新任务 → `explorer.exe`；安装本身不受影响。
- **首次安装也会重启一次**（复选框默认勾选）：此时没有旧 DLL 要换，是同一开关的副作用。
- **多会话机器**：`taskkill /im explorer.exe` 会命中所有可访问会话的 explorer。本机无法
  验证多会话行为；这是已知代价而不是静默行为（完成页复选框文案写明了会重启 Explorer）。
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
（§6.1 #12）—— 杀没杀掉、注册没注册、启没启起来、脚本有没有重启 Explorer，都还没人见过。

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

### 2.6 在 Windows 本机从零打包（2026-10-19）

> **结论先说**：`.msi` **只能在 Windows 上用 jpackage 打**。Linux 上的
> `cargo xwin build` 只能验证 TSF DLL 的交叉链接；Linux 没有可用的 jpackage MSI
> 目标，也不要用 Linux `gcab` 重打 CAB 当交付物（§2.5 已撤回）。
> Gradle/jpackage 产出的 MSI 仍**必须**过后处理（§2.5），否则不会注册 TSF。

**前提（Windows 10/11 x64）**：

- **JDK 21**（Temurin；`JAVA_HOME` 指向它），Gradle 用仓库自带的 `gradlew.bat`；
- **Visual Studio Build Tools**，勾选「使用 C++ 的桌面开发」（MSVC x64 + Windows SDK）
  以及 `dumpbin`；
- **Rust stable MSVC** toolchain（`rustup default stable-x86_64-pc-windows-msvc`）；
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

**第 2 步：打 desktop MSI**

```powershell
cd desktop
$env:OPI_TSF_DLL = (Resolve-Path ..\target\x86_64-pc-windows-msvc\release\tsf_opi.dll).Path
.\gradlew.bat packageMsi --no-daemon
```

`OPI_TSF_DLL` 必须在 `packageMsi` 前设置：`desktop/build.gradle.kts` 只在
该变量存在时把 DLL、`launch-and-restart.cmd`、`install-emoji-font.ps1` 和
`opi-emoji.ttf` 挂进 jpackage 的 `--input`。产物通常在
`desktop\build\compose\binaries\main\msi\opi-candidates-<版本>.msi`
（CMP 版本可能改路径，先 `Get-ChildItem -Recurse -Filter *.msi` 确认）。

**第 3 步：后处理（必需）**

**首选：Windows 原生 PowerShell**（本仓库 2026-10-19 新增，使用系统自带的
`WindowsInstaller.Installer` COM，不依赖 msitools/WSL）：

```powershell
# 回到仓库根目录（第 2 步 cd 进了 desktop）；以管理员身份运行 64 位 PowerShell
cd ..
powershell -ExecutionPolicy Bypass -File scripts\windows\postprocess-windows-msi.ps1 `
  -InputMsi desktop\build\compose\binaries\main\msi\opi-candidates-<版本>.msi `
  -OutputMsi dist\opi-<版本>_windows-desktop-processed.msi
```

它做与 `scripts/postprocess-windows-msi.sh` 相同的五张表修改（`INSTALLDIR=opi`、
797 杀进程、5900 Type 1 `regsvr32`、Finish 自启、`OPI_POSTPROCESSED` 标记），
并在 `Commit` 前做表级读回。⚠️ **该脚本尚未在真 Windows 主机上执行过**
（仓库的验证环境是 Linux），所以它是「本机打包的便捷路径」，不是已验证交付路径；
失败时不要退回原始 MSI 发布，改用下面这条路。

**已验证路径（CI/WSL）**：在 WSL/Debian 里装 `msitools` 后跑
`scripts/postprocess-windows-msi.sh`（命令见 §2.5）；CI 的
`opi-desktop-msi-v<版本>` 就是这条路产出的。两条路的表形状相同，bash 脚本有
全表 diff + cab 抽样解包；PowerShell 脚本只做读回，没有全表 diff。

**第 4 步：安装验收**

1. **先卸载旧包**（尤其是 v1.15.1 及更早的 raw MSI）：设置 → 应用 → 已安装的应用，
   或 `msiexec /x <旧包>`。旧包默认在 `C:\Program Files\opi-candidates`；新包默认
   在 `C:\Program Files\opi`。旧包不会自动清理 `HKLM\SOFTWARE\Microsoft\CTF`
   下的注册残留（§6.1 #13），如遇异常先按 #13 手工清理。
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
   `Get-WinUserLanguageList` 的 `InputMethodTips`、字体注册和安装日志里的
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
- **字体仍按用户装**：`install-emoji-font.ps1` 写运行者的 HKCU +
  `%LOCALAPPDATA%\Microsoft\Windows\Fonts`；SYSTEM 部署下需目标用户手工运行
  `INSTALLDIR\app\install-emoji-font.ps1`。
- 上述 HKLM 默认位在 Win10 上的实际效果**未在真机验证**。需要立即生效时，目标用户
  仍可自己跑一次 `regsvr32 /s "C:\Program Files\opi\app\tsf_opi.dll"`（写该用户 HKCU）。

**图标与字体落点（本轮）**：

- MSI 产品图标（`ARPPRODUCTICON`）与 `opi-candidates.exe` 图标：
  `desktop/icons/opi-pet.ico`（实测方法见 §2.5 的「图标」段）；
- PUA 字体源文件：`shared/pet/emoji-font/opi-emoji.ttf`；
- MSI 内落点：`INSTALLDIR\app\opi-emoji.ttf`（另有 jar 内
  `pet-font/opi-emoji.ttf` 供应用 classpath 直接加载）；
- 装机后（用户勾选 Finish）：`%LOCALAPPDATA%\Microsoft\Windows\Fonts\opi-emoji.ttf`
  + `HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts`。静默安装不会执行，
  需要时手工运行 `INSTALLDIR\app\install-emoji-font.ps1`。

> **必须新构建，旧包不会变**：2026-10-19 用户在真机装的三个 MSI
> （本地 `processed-static`、本地 `processed`、CI `opi-desktop-msi`）里的
> **desktop jar 都是旧构建**；本轮改的是源码（TSF 注册、界面、字体接线），
> 这些包不会因为仓库源码变了而改变。要验证本轮改动，必须按本节重新
> `packageMsi` 并重新后处理；旧包只能用来证明旧行为。

---

## 3. 安装与注册

### 3.1 注册命令

> **2026-10-18 起（2026-10-19 收紧）**：**后处理过的** MSI 会自动执行 `regsvr32 /s`
> （后处理脚本的 `OpRegisterTsf`，序列 5900），**用户不再需要手工注册**。
> ⚠️ Release 里 v1.15.1 及更早的 Windows MSI 是**未后处理**的原始产物（不注册）；
> 2026-10-19 起 CI 的 `opi-desktop-msi-v<版本>` artifact 才是处理过的版本。
> 本节保留为核对/回退路径：想确认注册结果、或从旧版单独 DLL 迁移时，仍可按下面的
> 命令手工跑一次。注意 `regsvr32` 写的是**当时 DLL 的全路径**；后处理后的 MSI
> 默认装在 `C:\Program Files\opi\app\tsf_opi.dll`（2026-10-19 起；此前是
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
- 注册后建议重启一次宿主程序 / 重新登录，让 TSF 重新枚举输入法。**【未验证】**

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
- `ITfCategoryMgr::RegisterCategory`，类别 = `GUID_TFCAT_TIP_KEYBOARD`（"键盘输入法"）。
  **调用前用 `EnumItemsInCategory` 枚举**，已有本 CLSID 就跳过。

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
列表里没有时，先跑 §2.6 第 5 步的 `scripts/windows/diagnose-tsf.ps1`：它会打印
`Get-WinUserLanguageList` 的 `InputMethodTips`、HKLM/HKCU 的 CTF profile 与
`Enable`，以及安装日志里的 `OpRegisterTsf`。

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
| 上下页 | `PageUp` / `PageDown`；组字中 `-` / `=` | `KEY_PAGE_UP` / `KEY_PAGE_DOWN` 分支；`-`/`=` 见 `handle_printable` 顶部（2026-10-09 补） |
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
3. **逐字确认（2026-10-17）**：整串无词组精确匹配时（前缀扩展不算），点选先「确认一个字」——它留在候选窗
   的缓冲显示里（管线协议 `show.buffer` 现在带「中guo」这类片段，`TsfLogic::display_buffer`），
   最后一段确认才一次提交整串；点中间的字不会立刻插入，这是语义不是缺口（引擎级，
   门禁 `tests/segment_commit.rs`）。
4. **目标文档取自上一次键事件。** 点了候选、还没按键就换文档，字会落进**上一个**文档
   （`tsf.rs::drain_pending` 的注释自己记了这条"已接受的边缘"）。
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
opi-candidates.exe --settings     # 设置窗
opi-candidates.exe                # 候选窗（无参数；TSF 走命名管道驱动它）
```

设置窗保存后**只落盘**。⚠️ **生效时机 = 输入法下一次激活**（切到别的程序再切回来）——
TSF 侧**没有在线刷新通道**：`tsf.rs` 的 `Activate` 会重读一次这个文件
（`logic.apply_settings`）。升级路径：设置窗与候选窗是同一条命名管道的两端，加一条
`{"type":"reload"}` 消息即可即时生效 —— 要同时改线协议两侧（`candidate_io.rs` 与
`desktop/.../Protocol.kt`），**本轮没做**。

⚠️ **2026-10-12 布局改过**（用户裁决「布局和功能参考主流输入法」）：设置窗从单列长滚动
改成**左分类导航 + 右内容 + 底部状态行**（与 Linux 同一份代码，`SettingsWindow.kt`）。
Windows 上只出**常用 / 状态栏 / 符号与表情 / 关于**四页 —— **按键**（`ShiftToggle`）与
**外观**（`Skin` 是 fcitx5 主题）两页不出：`crates/tsf-opi/src/settings.rs` 的读侧不认
这两个键（「宁可少一页，不做点不动的开关」；过滤是纯函数 `settingsTabs`，有单测钉）。
「符号与表情」页那两个按钮打开的是与 Linux 同一份代码的窗（`SymbolsPage.kt` 的
`SymbolWindow`，表情页在 `EmojiPage.kt`）：页面能看能翻，但**点选上屏会失败并如实
显示原因** —— 上屏要命名管道，而管道被候选窗进程持有（单实例），设置窗进程送不出去
（`desktopSymbolCommit` 的 Windows 分支）。表情页布局与 Android `keyboard/EmojiPanel.kt`
统一：顶栏 `[← 返回] [自有 | Emoji | 颜文字] [搜索框]`；**自有**是独立子页，4 列网格
显示 `shared/pet/stickers/` 的 12 张小欧 PNG，点击 = 复制图片到系统剪贴板（不经过
命名管道、不提交文本），因此**在设置窗里也能用**；Emoji = 9 个 OpenMoji 分组（不再
含自有），颜文字 = `data/raw/kaomoji.tsv` 的 7 组（`copyEmojiData` /
`copyKaomojiData` / `copyPetStickers` 在构建期把数据与 PNG 拷进程序资源，见
`desktop/build.gradle.kts`）。上面那段 `settings.ini` 的四个键不受影响（照旧四个）。

**「属性」入口已接线**（2026-10-08）：服务对象实现了 `ITfFunction` + `ITfFnConfigure`
（`tsf.rs`），`Show()` 会拉起设置窗（`configure.rs`：从 **DLL 同目录** →
`%LOCALAPPDATA%\opi\` 逐候选找 `opi-candidates.exe`，都找不到就**什么都不弹**）。
控制面板那一页取它的办法是 `CoCreateInstance(CLSID, IID_ITfFnConfigure)`（MSDN 的
`ITfFnConfigure` 页面）——⚠️ **本机只验到 `cargo check`**，真机上「属性」按钮会不会出现、
点了起不起得来，都要在 Windows 上看（§6.1 有这一条）。

⚠️ 仍然缺的：**Windows 没有打包**（§2.4）—— 设置窗没有安装包，`opi-candidates.exe`
要用户自己放到上面那两个候选位置之一，「属性」才拉得起来。

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

**【未验证】** —— **本机（Linux）没有在真机桌面 Windows 上装过任何后处理产物**；
CI 的 `windows-install-test` 只做容器冒烟并抓日志（§2.5），不是真机验证。
2026-10-19 用户真机报障（v1.15.1，未后处理包：有进程、键盘不显示）是本轮修复的
**直接证据**；但「后处理后的包能不能注册成功」仍要装一次才知道。装完后按顺序核：

> ⚠️ **先处理旧包**：如果机器上已经装过 v1.15.1 的**原始** MSI（默认在
> `C:\Program Files\opi-candidates`），请先卸载它再装后处理包。新版把默认目录改成
> `C:\Program Files\opi`，而 jpackage 没有显式设置 `--win-upgrade-uuid`；两者并存/
> 升级的行为未在真机验证过，同版本号也可能被 Windows Installer 当成维护/修复而不是
> 新安装。卸载旧包不会删除 `%LOCALAPPDATA%\opi` 里的用户数据。

> ⚠️ **Win10 二次安装/覆盖升级必须单独验证**（2026-10-19 复审要求）：在同一台机器上
> 把**同一个后处理包再装一次**（或覆盖升级到更高版本），确认 `OpRegisterTsf` 仍成功、
> 安装不回滚、语言列表里输入法不重复也不消失。注册代码已避免重复写已存在的 profile、
> `EnableLanguageProfileByDefault` 失败时回读 HKLM `Enable` 容错、HKCU 改为 best-effort，
> 但**这些路径都没有真机证据**（本仓库只在 Linux 上验证）。若二次安装失败，先看
> `install.log` 里 `OpRegisterTsf` 前后的错误码。

0. **一次性诊断（推荐先跑）**：装完运行 §2.6 第 5 步的
   `scripts/windows/diagnose-tsf.ps1 -InstallLog .\install.log`。它只读，退出码
   0/1 直接说明必需项是否齐全；下面几项是它的手工版本。
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
3. **`regsvr32` 本身成功**：安装时它由 `WixSilentExec64`（`/s` 静默）执行，完成页看不到
   输出。注册 CA 是 **Type 1（Return=check）** ⇒ 注册失败会让**安装失败并回滚**，
   不是「装成功、键盘不出现」。排障看安装日志（`msiexec /i <包> /l*v install.log`）里
   `OpRegisterTsf` 与 regsvr32 的返回码；也可在装完后手工跑一次
   `regsvr32 "C:\Program Files\opi\app\tsf_opi.dll"`（应弹成功框）。
   ⚠️ 旧构建（如 v1.15.1 包内 DLL）动态依赖 `VCRUNTIME140.dll`：没装 VC++ 运行时的
   机器上这一步会以 `0x8007007E` 失败 —— 新 CI 产物已改静态 CRT（§2.5 第 1 条）。
4. **键盘列表出现「小欧输入法」**：§3.3 的第 1–3 步（中文(简体，中国) → 语言选项 →
   键盘），或 `Win`+`Space` 轮换。**这是端到端判据**：前三步都对但列表里没有，说明
   TSF profile 没注册上，回看第 2、3 步。

TSF 那一半是官方 API 写的，**没有"预期键路径"可对照** —— 源码有意如此（见 §3.2）。

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
| 2 | ~~`tsf_opi.dll` 从未被链接出来过，导出表从未被检视~~ **【已消除 2026-10-09】** 已在 windows runner 上真链接、导出表实测四个 COM 入口（名字未装饰，§2.5）。**【订正 2026-10-18】** DLL 已随 MSI 进入 File 表，安装序列会跑 `regsvr32`；但**真机上的注册结果、宿主加载**仍从未发生 —— 「包里有/表里有」与「能用」是两格 | §1.2、§2.5 |
| 3 | `DllRegisterServer` / `DllUnregisterServer` 的注册效果、管理员判定 | §3.1 |
| 4 | TSF 能否认出这个 TIP、键盘列表里出不出现。**【订正 2026-10-19 复审】** `register()` 已补 `ITfInputProcessorProfileMgr::RegisterProfile`（判存在 + `bEnabledByDefault=TRUE`）、`ITfInputProcessorProfiles::EnableLanguageProfile` 与 `EnableLanguageProfileByDefault`；后处理改用 64 位 `WixSilentExec64`。但仍未在真 Windows 上装过修复后的包，所以「能出现在列表」仍属未验证 | §3.2、§3.3 |
| 5 | 按键回调、composition、文本插入的**全部运行期行为** | §5.3 |
| 6 | 候选窗与插件的命名管道连通（含 `\\.\pipe\opi-candidates` 单实例语义） | §4.3 |
| 7 | 四个 `cfg(windows)` 模块**没有任何测试** —— 只有 §1.2 那条 `cargo check` 看过它们 | §1.2 |
| 8 | 候选窗在 Windows 上的~~构建与~~**运行**：**构建已被 §2.5 覆盖**（.msi 真在 windows runner 上打出来了），**跑起来仍然一次都没有** | §2.3、§2.5 |
| 9 | `Ctrl+'` / `Ctrl+\` / `Shift+Space` 在 Windows 上有没有被别的软件占用 | §4.1 |
| 10 | 语言栏按钮（`langbar.rs`）的运行期行为：`AddItem` 成不成、`GetIcon` 有没有被调、图标画没画出来 —— 只有 §1.2 那条 `cargo check` 看过它（"挑条目 + 校验 RT_ICON 形态"那半在 `langbar_icon.rs`，有主机单测） | §2.1、`crates/tsf-opi/src/langbar.rs` 头注释 |
| 11 | **「属性设置」的生效时机**：设置窗落盘后要等下一次 `Activate` —— 本机无法验证（`Activate` 由 TSF 调）。读取/容错/落点那半在 `settings.rs`，有主机单测 | §4.4、`crates/tsf-opi/src/settings.rs` |
| 12 | **安装器四条新行为（改安装目录 / 关停运行中实例 / 注册 TSF DLL / 装后自启 + 重启 Explorer，2026-10-17 / 10-18 / 10-19 加）**：表级已改好并可本地复核（§2.5 的后处理脚本 + 读回证据）。**2026-10-19 用户在真机装过的是「未后处理」的 v1.15.1 发布包**（有进程、键盘不显示）—— 这证明了「不跑后处理 = 不注册 = 键盘不出现」。**【订正 2026-10-19 复审】后处理后的三份包**（本地 `processed-static`、本地 `processed`、CI `opi-desktop-msi`）**仍无输入法的根因已定位**：注入的是 32 位 `WixSilentExec`，而随包 `Binary.WixCA` 是 PE32 i386 ⇒ WOW64 调 SysWOW64 的 32 位 `regsvr32`，加载不了 x86-64 DLL。脚本/CI 已改为 `WixSilentExec64` + `WixSilentExec64CmdLine` 并加 PE machine 断言，注册逻辑改为显式判存在 + HKLM 默认启用。但这三份包的 desktop jar 与 DLL 仍是本轮修复前的构建，所以「修复后的包」仍未被**真机**装过（CI 容器冒烟只抓日志，§2.5）：taskkill 真杀没杀掉、`regsvr32` 真注册没成功、WixShellExec 真启没启起来、ExitDialog 复选框出不出现、脚本是否结束 Explorer 并由 Winlogon 拉回（依赖 `AutoRestartShell` 默认 1），仍是【未验证】 | §2.5、`scripts/postprocess-windows-msi.sh` |
| 13 | **卸载不清理 TSF 注册（2026-10-19 reviewer m4 记录，有意未做）**：`OpRegisterTsf` 的条件是 `NOT REMOVE` ⇒ 卸载**不执行** `regsvr32 /s /u`；`HKCR\CLSID\{98C51858-590B-404D-8915-21EB44250062}\InprocServer32` 与 `HKLM\SOFTWARE\Microsoft\CTF` 下由 `DllRegisterServer` 写入的条目会**残留并指向已删除的 DLL**。未加卸载动作的原因：`WixSilentExec64` 只有**一个**命令属性 `WixSilentExec64CmdLine`，安装/卸载两条命令会互相覆盖；手写 `RemoveRegistryKey` 又需精确复制官方 API 写入的键，未在真机取证。重装到同一路径会重新覆盖注册；彻底清理 = 卸载前手工 `regsvr32 /s /u <dll>`，已卸载则手工删上述键 | `scripts/postprocess-windows-msi.sh`、§2.5 | 卸载后注册残留；卸载本身的文件删除不受影响 |
| 14 | **Win7 / Win8 / 8.1 不支持（2026-10-19 用户裁决：仅文档标注）**：DLL 的 API set/Rust std 与候选窗的 JDK 21/Compose 下限都在 Win10/11；Win7/8 需另建 legacy 轨道 | §1.1 的说明 | 在 Win7/8 上安装/运行不保证；不在本仓库范围 |

### 6.2 代码里已知的缺口（读源码得出，不是待办猜测）

| # | 缺口 | 依据 | 影响 |
|---|---|---|---|
| 1 | **词库仍没有打包步骤**：MSI 已带 DLL（2026-10-18），但 `luna.opid` 仍不会被放到 DLL 旁边 | §2.4 | 开箱只有内置回退词库 |
| 2 | **没有人拉起候选窗进程**；~~候选窗也没有 Windows 安装包~~ **【订正 2026-10-09】** 安装包有了（§2.5 的 `opi-desktop-msi-*` artifact），但它在 Release 之外、90 天过期。**【订正 2026-10-17】** 后处理后的 `.msi` 里「装完点 Finish 自启」已接线（§2.5）—— 但**这条线本身未在真机验证**。**【订正 2026-10-19】** 经 v1.15.1 实物 MSI 核实：到 v1.15.1 为止的 **Release 资产全部是未后处理**的原始包（无注册动作；用户真机报障即此） | §4.3 #1、§2.5 | 候选窗看不到（打字仍可用），直到有人装上后处理的包并勾选 Finish |
| 3 | **点选延迟到下一个键事件才插入** | `state.rs` 模块头、`tsf.rs::drain_pending` | 点了不敲键 = 字不出现 |
| 4 | `OnSetFocus` 空实现（drain 的天然落点没接） | `tsf.rs` 的 `OnSetFocus` | 同上，放大了 #3 |
| 5 | 点选后换文档 → 插到上一个文档 | `tsf.rs::drain_pending` 注释 | 边缘场景 |
| 6 | **`Deactivate` 是骨架**：没有 `UnadviseKeyEventSink`，没有释放 composition / 候选窗 | `tsf.rs::Deactivate`；`docs/superpowers/specs/2026-08-14-opi-multi-platform-design.md` 的「tsf-opi Deactivate 为骨架（正式范围降级）」一行，明示为"验收补全点" | 待真机验收补 |
| 7 | 装载日志走 `eprintln!`，GUI 宿主里通常看不见；`OutputDebugStringW` **未实现** | `com_server::CreateInstance` 注释 | 排障全靠间接观察 |
| 8 | **繁体（`trad.opid`）在 Windows 上无从装载**：`dict_path::candidates` 里没有它，且 `TsfLogic` 只持一个词库 | `dict_path.rs` 模块头的「已知缺口」 | 繁体模式打不开（与 README「Windows 没有繁体入口」一致） |
| 9 | 模式键**丢弃未提交缓冲**（不先上屏） | `tsf.rs::toggle_mode` → `Engine::switch_mode`；`engine.rs::toggle_symbol` 注释 | 打到一半切模式 = 丢字 |
| 10 | 候选窗单页显示上限 `MAX_CANDIDATES = 8` 与 Rust `logic::PAGE_SIZE` 同为 8，**但两者之间没有门禁**（Rust 那份有 `const` 断言绑 `engine-core`，Kotlin 那份只有一句注释说"一致"） | `CandidateWindow.kt` vs `logic.rs` | 若 `PAGE_SIZE` 改了，窗口会静默少显示最后一个候选 |
| 11 | **模式热键不认键盘布局**：`vk::mode_hotkey` 比的是**裸 VK**（0xDE / 0xDC），即 US 布局上标着 `'` 与 `\` 的那两个物理键位；而字符键走的是布局感知的 `ToUnicodeEx`（`tsf.rs::to_engine_keycode`）。⇒ 非 US 布局（AZERTY / Dvorak）上，要按的是**那个位置**的键，不是键帽上的字符 | `vk.rs::mode_hotkey` 的 `VK_OEM_5` / `VK_OEM_7`；`tsf.rs::to_engine_keycode` | 非 US 布局用户会"按了没反应"（**未验证**：本机没有任何非 US 布局可测） |
| 12 | 语言栏按钮**点击无动作**（有意：不假装有功能）；DLL **没有文件图标**（`.rc` / 资源编译器在本机与 CI 都不可验证 ⇒ 走 `include_bytes!`，代价就是缺这一块） | `langbar.rs` 的 `OnClick` 与模块头「为什么不用 .rc」 | 点击没反应是设计如此；资源管理器里 DLL 显示默认图标 |
| 13 | **`ITfFnConfigure` 只过 `cargo check`**：控制面板里那个「属性」按钮会不会出现、点了拉不拉得起设置窗，本机无法验证；拉起的候选路径判定有主机单测（`settings_exe.rs`），Win32 那一步没有 | §4.4、`crates/tsf-opi/src/configure.rs` | 真机上「属性」可能不出现/不响应 |
| 14 | ~~**设置窗没有 Windows 安装包**~~ **【订正 2026-10-09】** 安装包有了：`desktop/build.gradle.kts` 的 `targetFormats` 加了 `Msi`（原来只列 `Deb`），`.github/workflows/windows.yml` 在 Windows 主机上真打 `.msi`（§2.5）。剩下的缺口：**不在 Release 里、未签名、artifact 90 天过期**，拿到手得自己去 Actions 取 | §4.4、§2.5 | 装完也不会自己出现（本表 #2：没人拉起它） |
| 15 | **W1「切中→英直接确认」在 Windows 侧只有状态机**（2026-10-10；同日深夜语义改为**一律原文**——拼音/繁体上缓冲原文、符号模式的关键字只清不提交）：`switch_mode_confirmed` 与引擎 `confirm_pending` 已就位并有两轨配对测试 + 负对照，但 `tsf.rs::toggle_mode` 的调用点**有意未接线**（插入面未通电；接线 = 改调它 + 把返回值插入文档）⇒ 现状仍同 #9 | `logic.rs`、`logic_input_method.rs`、`tsf.rs::toggle_mode` | 行为与 #9 同：打到一半切模式丢字 |
| 16 | **桌面三件套在 Windows 上「代码就绪、地基未通电」（2026-10-10）**：符号页能看能翻但**无上屏通路**（命名管道单实例被候选窗进程持有，状态栏进程发不出 `commit_text`）；状态栏 `--statusbar` **无拉起触发器**；符号数据未随 MSI 铺设 ⇒ 页面如实显示「没有类目数据」。单进程拓扑（候选窗/状态栏/符号页同进程）与 TSF 拉起命令都归「Windows 接线轮」 | `desktop/` 的 `SymbolsPage.kt`/`StatusBar.kt`/`Main.kt` 文件头注释 | Windows 用户看不到这些新面（不是挂了，是没通） |
| 17 | **虚拟键盘 Windows 路径（`SendInput`）只过编译**：本机无 Windows，运行期未验证；一并如实记：Linux 的 XTEST 注入与状态栏窗口**也未做 GUI 冒烟**（本机无 Xvfb，待真桌面时间窗）——逻辑层（键表物理键码、可见性组合、注入序列映射）已测 | `desktop/` 的 `VirtualKeyboard.kt`、`docs/install/linux.md` §4.8 | 未验证 ≠ 不可用，但也别当已验证 |

### 6.3 交付这件事本身

**Windows 端目前没有可下载产物**（Release 不带 Windows 包），且**本文档是这一端的第一份安装文档**。
「仓库无安装文档」此前是审计记录在案的共同拦路项之一 —— 本文件是它的第一部分，
但**只覆盖 Windows 一路，且未经验证**。

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
> 64 位 `WixSilentExec64` 后处理、界面、PUA 字体接线的改动，**不会**出现在这些旧包里。
> 要验证本轮改动，
> 必须按 §2.6 重新 `packageMsi` 并重新后处理；旧包只能证明旧行为。
> `processed-static`（Linux `gcab` 重打 CAB）已撤回，不再是交付物。
