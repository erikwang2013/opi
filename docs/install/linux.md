<!-- SPDX-FileCopyrightText: 2026 erik.xyz -->
<!-- SPDX-License-Identifier: MIT -->

# Linux 端（fcitx5 插件）编译 / 打包 / 使用

> ### 先读这一段
>
> **Linux 是三个「有端代码」的平台（Android / Linux / Windows）里，唯一有 CI 在真
> fcitx5 上验到「装完会不会被加载」这一步的**——依据是 `.github/workflows/ci.yml` 的
> `fcitx5` job（真编 + 在 debian:12 容器里起真 fcitx5，看它有没有加载本 addon）。
> `README.md`「项目状态」里 M6b 仍写着「待在目标平台实机验收」，那指的是**产品验收**；
> 编译、安装落点、addon 能否加载这三件事已有实测结论，出处是那个 job 与
> `crates/fcitx5-opi/cpp/CMakeLists.txt`。
>
> ⚠️ 但**真桌面前端（GTK/Qt 经 IM 模块收字）下的实际输入没人跑过** —— 见 §8 最后
> 第二条。别把「CI 绿」读成「你自己那台机器上一定能打字」。
>
> 每一条都带证据档位：
>
> | 标记 | 含义 |
> |---|---|
> | **【实测】** | 2026-09-28 在本机（Deepin 25）或本机 docker 容器里真跑过，命令与输出在文内 |
> | **【推断】** | 从源码读出来的结论，给出文件 + 符号；没有运行过 |
> | **【未验证】** | 本机够不到（授权镜像 / 没有该发行版），**没有任何人跑过这一步** |
>
> 本项目既有教训是「写完了、读起来像完成、实测却发现从未被编译器看过」——所以
> 本文凡是带数字的地方都写明**哪条命令能重新量一遍**，而不是让你信这句话。

---

## 1. 三个硬门槛

装之前先确认这三条。它们不是「建议」——**第一行不满足时不会给你报错**，只会
「装完了没反应」。

| 门槛 | 值 | 权威在哪（改之前先读那里） | 不满足会怎样 |
|---|---|---|---|
| fcitx5 | **≥ 5.0** | `crates/fcitx5-opi/data/addon/opi_fcitx5.conf` 的 `[Addon/Dependencies]` 那一行 | 输入法**能选中、一个字都不出**，且日志里**没有任何 opi 相关报错** |
| Rust | **≥ 1.88** | `Cargo.toml` 的 `[workspace.package] rust-version`（CI 的 `msrv` job 按它真编一次） | 编译期红。这条会响，不静默 |
| 编译器 / 构建 | **C++20** + **cmake ≥ 3.16** | `crates/fcitx5-opi/cpp/CMakeLists.txt` 的 `cmake_minimum_required` 与两个 `CXX_STANDARD` | 编译硬错误（与 `-Werror` 无关） |

### 1.1 先查你自己的 fcitx5

```bash
fcitx5 --version        # 打版本号就退出
```

版本低于 5.0 的话，**插件侧没有兼容开关** —— 那个门槛不是插件自己判的，是 fcitx5
拿 `opi_fcitx5.conf` 里的声明去比的，比不过就**整个 addon 不加载**。所以唯一的路是
升级发行版或换一个带新 fcitx5 的源。

> **为什么这条值得单列一节**：`data/addon/opi_fcitx5.conf` 里那段注释记着实测 —— 同一批
> `.so`，**只改门槛那一行的值**：写高了，fcitx5 日志里 `Loaded addon opi_fcitx5` 出现
> 0 次；写对了 1 次。用户侧的观感就是「输入法选得到、一个字都不出」，而**日志里没有
> 任何报错**。这也正是 CI 那条门禁不用文本断言（「conf 里的数字 ≤ 最老发行的版本」）
> 而用行为断言（加载了才算过）的原因。

### 1.2 Rust 工具链：Debian/Ubuntu 系**必须**用 rustup

`rust-version = "1.88"` 卡的是 `let`-chain（1.88 才稳定）；`edition = "2024"` 本身只要
1.85，真正压住下界的是前者。依据是 `Cargo.toml` 同一段的注释（把上游改掉后重编，实测
三处触发点）。

**2026-09-28 实测各发行版自带 rustc**（在本地 docker 镜像里用各发行版自己的包管理器查
出来的，不是转述）：

| 发行版 | 自带 rustc | 够 1.88 吗 |
|---|---|---|
| Debian 13 (trixie) | 1.85.1 | **✗** |
| Ubuntu 24.04 | 1.75.0 | **✗** |
| Arch | 1.98.1 | ✓ |
| Fedora 44 | 1.98.1 | ✓ |
| openSUSE Leap 16 | 1.98.0 | ✓ |

⇒ **准确的结论不是「发行版自带的基本都不够」，而是「Debian/Ubuntu 系不够，滚动发行版够」。**
（这张表会漂。用它自己那条命令重查：`apt-cache policy rustc` · `pacman -Si rust` ·
`dnf list --available rust` · `zypper search -s rust`。）

⚠️ **别拿报错文本判自己需要哪一版**：1.85 / 1.86 / 1.87 的报错**完全一样**（E0658 一屏
解析错），1.88 起才会明确写「requires rustc 1.88」。

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustc --version        # 需要 ≥ 1.88
```

---

## 2. 编译

### 2.1 装构建依赖

只列**编译**需要的。运行时依赖只有 `fcitx5` 本身，且**带版本约束**（`>= 5.0`）——
见 `scripts/nfpm.yaml` 的 `overrides`（deb 与 rpm 的依赖语法不同，所以按格式分开写：
deb 是 `fcitx5 (>= 5.0)`，rpm 是 `fcitx5 >= 5.0`）。

**Debian / Ubuntu / Deepin 系**【实测：包名在 debian:12、debian:13、ubuntu:22.04、
ubuntu:24.04 四个仓库里逐个 `apt-cache policy` 查过，都存在】

```bash
sudo apt-get install build-essential cmake \
    libfcitx5core-dev libfcitx5utils-dev libfcitx5config-dev
```

⚠️ 三个 `-dev` **缺一不可**：`libfcitx5core-dev` 的头会 include 到 `Config`，少一个就在
`fcitx/addoninstance.h` 报 `fcitx-config/configuration.h` 找不到。CI 里装的也是这三个
（`ci.yml` 的 `fcitx5` job）。

**RPM 系**【实测：在两个发行版的仓库里查过】

```bash
sudo dnf install gcc-c++ cmake fcitx5-devel      # Fedora
sudo zypper install gcc-c++ cmake fcitx5-devel   # openSUSE Leap
```

⚠️ 两族的 `-dev` 包名**不一样**：Debian 系是三个 `libfcitx5*-dev`，RPM 系是单个
`fcitx5-devel`。

⚠️ **openEuler 装不了**：实测 `dnf list --available fcitx5` 返回
`No matching Packages to list`，而同一仓库里 `ibus` 是有的 ⇒ 它的中文输入走 ibus。
**这不是配置问题，是那个发行版没有 fcitx5。**【实测】

**Arch**【实测：在 `archlinux` 容器里 `pacman -Sy` 后 `pacman -Fl fcitx5` 看过文件清单】

```bash
sudo pacman -S --needed base-devel cmake fcitx5
```

Arch 的 `fcitx5` **单包自带头文件与 CMake 配置**，没有单独的 `-devel`：实测该包含
`usr/include/Fcitx5/{Core,Utils,Config}/`、`usr/lib/cmake/Fcitx5*/`、
`usr/lib/fcitx5/`（addon 目录）。

**核不到包的发行版**：BlackArch / UOS / 银河麒麟 —— 见 §8。RPM 族的包名我在
Fedora 与 openSUSE Leap 两家仓库里查到了；**其它 RPM 发行版（Rocky / Alma / CentOS
Stream 等）没有逐个核**，照 `fcitx5-devel` 试，试不到就是 §8 那一栏。

#### 不装 `fcitx5-dev` 也能编（解包头那条路）—— 有个已知待修

不想装开发包（或在没 root 的机器上）时，可以从发行版仓库下载 `.deb` 解包出头/库，
再用 `-DFCITX5_HEADERS=… -DFCITX5_LIBS=…` 指过去（做法与 `cpp/README.md`、
`cpp/run-harness.sh` 头部注释里那套一致）。**但这条路目前有一个已知缺陷**：

`crates/fcitx5-opi/cpp/CMakeLists.txt` 的 `FCITX5_HEADERS` 分支里，
`target_include_directories` **少了 `SYSTEM`** ⇒ 出来的是普通 `-I` 而不是 `-isystem`，
于是 fcitx5 头文件**内部**引用它自家弃用别名时那条诊断**追不回**（两个 `.cpp` 里的
`#pragma` 抑制写在 include **下方**，管不到头内部）⇒ **在 5.1.21 及以上的头上会硬红**。

⚠️ **本机走的就是这条回退路，只是本机头还是 5.1.12 所以没暴露。** 修法已验证（加
`SYSTEM`），**已知待修**。在那之前，走这条路请优先用 §2.1 那三个包（`find_package`
分支没有这个问题）。

### 2.2 编译与安装

```bash
git clone https://github.com/erikwang2013/opi.git && cd opi

# 1) Rust 侧 cdylib → target/release/libfcitx5_opi.so
cargo build --release -p fcitx5_opi

# 2) C++ 胶水 + 安装规则
cmake -S crates/fcitx5-opi/cpp -B build-fcitx5 \
      -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_INSTALL_PREFIX=/usr
cmake --build build-fcitx5
sudo cmake --install build-fcitx5
```

⚠️ **`CMAKE_INSTALL_PREFIX` 必须是 `/usr`**，这不是洁癖：addon `.so` 的落点由 fcitx5
自己的 `FCITX_INSTALL_ADDONDIR` 决定，而它跟着前缀走 —— 前缀是缺省的 `/usr/local` 会
得出 `/usr/local/lib/fcitx5`，而发行版 fcitx5 编译进去的搜索路径里**没有这一条**。
`CMakeLists.txt` 在 configure 期会为此报一条 `WARNING`（**配置期的警告有执行点，写在
文档里的提醒没有**）。

**词库不用你生成**：CMake 缺省取**仓库里已入库**的
`android/app/src/main/assets/luna.opid`，不存在「先跑个脚本」这一步。想换词源才需要
生成，见 §5。

`cmake --build` 除了编胶水，还会编一个**落点探针** `build-fcitx5/opi_locate_check`：
它问**真 fcitx5 库**「`loadDictionary()` 会去哪找词库」，再与本次 install 的落点比。
它进 `ALL` 目标，所以每次 `cmake --build` 都编 —— 不是可选步骤。

### 2.3 装了什么

下面是**包里实测的清单**（`dpkg-deb -c dist/opi_<版本>_<架构>.deb` 与
`rpm -qlp dist/opi-*.rpm` 的真实输出，不是设计的清单 —— 想复核就自己跑这两条，
别信这张表）：

| 落点 | 内容 | 谁在找它 |
|---|---|---|
| `/usr/lib/<arch>/fcitx5/`（RPM 系是 `lib64/fcitx5/`） | `libfcitx5_opi.so`（Rust 逻辑）+ `libfcitx5_opi_glue.so`（C++ 胶水） | fcitx5 的 addon 加载器 |
| `/usr/share/fcitx5/addon/opi_fcitx5.conf` | addon 元数据 + 门槛 | `Type::PkgData + "addon/…"` |
| `/usr/share/fcitx5/inputmethod/opi.conf` | 输入法条目 | `Type::PkgData + "inputmethod/…"` |
| `/usr/share/opi/luna.opid` | 词库 | `Type::Data + "opi/luna.opid"` |
| `/usr/share/doc/opi/LICENSE` + `LICENSES.md` | 许可证文本 | 人（授权要求，见 §6） |

⚠️ **两个 `.so` 必须同目录**：胶水用 `$ORIGIN` rpath 找旁边那个 Rust cdylib，
拆开放就加载不了。

⚠️ **`/usr/share/opi/` 上没有 `fcitx5` 中间层**，别顺手把它挪到 `share/fcitx5/` 下面。
`CMakeLists.txt` 记着这个坑：词库用的是 `Type::Data`（**不带** fcitx5 前缀），挪错了是
**找不到且不报错**，只表现为候选质量骤降 —— 与「压根没装词库」在用户侧无法区分。

---

## 3. 打包（`.deb` / `.rpm`）

```bash
scripts/build-packages.sh              # 两个都出
scripts/build-packages.sh deb          # 只出 .deb
scripts/build-packages.sh rpm          # 只出 .rpm
scripts/build-packages.sh --libdir lib64 rpm
```

产物落 `dist/`。前提只有一个：`nfpm` 在 `PATH` 里（脚本头部会告诉你三种装法）。脚本
**自己不做**版本号声明 —— 版本取自 `Cargo.toml` 的 `[workspace.package] version`，单一
真源。

设计上有一条值得知道，因为它决定了「包里的路径为什么是对的」：

> **落点不在打包脚本里声明。** 包里的文件树 = `cmake --install` 的暂存树**原样搬过去**，
> 而那个落点被 `opi_locate_check` 对着真 fcitx5 库验过。打包侧再抄一份路径 = 多一个
> 会漂的真源。两种格式的库目录本来就不同（Debian 多架构 `lib/x86_64-linux-gnu`，
> RPM 系 `lib64`），所以脚本**分别构建两棵暂存树**。

脚本里还有一道**空转护栏**：暂存树文件数低于脚本里写死的那个下限（两个 `.so` + 两个
conf + 词库）就拒绝打包 —— 构建不完整时宁愿不出包，而不是安静地打出一个残缺的包。

### 3.1 安装 / 卸载

```bash
sudo dpkg -i dist/opi_<版本>_<架构>.deb        # Debian 系
sudo rpm -Uvh dist/opi-<版本>-1.<架构>.rpm     # RPM 系（-U 兼顾首次安装与升级）
sudo apt-get -f install                        # Debian 系缺依赖时补
```

如果你机器上的 fcitx5 低于 5.0，**`apt`/`rpm` 会当场报依赖不满足而拒绝安装** —— 这是
**故意**的：把「装上了但静默不工作」换成一次显式拒绝。`nfpm.yaml` 里为此专门写了按格式
分开的版本约束，读那段注释能看到理由。

卸载就是标准的 `dpkg -r opi` / `rpm -e opi`。**本插件自己不写任何用户状态**（学习结果
目前不落盘，见 §7），所以 `~/.config/fcitx5` / `~/.local/share/fcitx5` 下不会有本插件
留下的东西要手清 —— 那里若还有内容，属于 fcitx5 自身或别的 addon。

---

## 4. 使用

### 4.1 装完必须重启 fcitx5

```bash
fcitx5 -r -d      # -r = replace，重启守护进程
```

`fcitx5 --help` 里 `-r, --replace` 的含义就是「替换正在跑的那个实例」——这条是实测过的
（`fcitx5 --help` 的输出），也是本文唯一敢写进步骤的写法。

⚠️ 「能不能不重启、只重载一下配置」**本文没有验过**（见 §8），所以没写。装完没反应的
第一件事就是老老实实 `-r` 一次，别先怀疑插件。

### 4.2 添加 OPI

```bash
fcitx5-configtool
```

「输入法」页 → 找到 **OPI** → 加进去 → 应用。

> 列表里看不到它时，那句「只显示当前语言」的过滤项是常见嫌疑 —— 但**本机没有图形
> 会话，这一句 UI 描述未经验证**【未验证】。看不到就先按 §10 的第一行查文件在不在。

### 4.3 键位

模式热键判在 Rust 路由**之前**（`crates/fcitx5-opi/cpp/opi_fcitx5.cpp` 的
`handleModeHotkey` / `handleFullwidthHotkey`）。三个键都在**同一个模式上来回切**，
不是单向进入：

| 键 | 作用 |
|---|---|
| `Ctrl+'` | 英文 ⇄ 拼音 |
| `Ctrl+\` | 符号 ⇄ 拼音 |
| `Shift+Space` | 全角 ⇄ 半角 |

其余按键路由在 `crates/fcitx5-opi/src/input_method.rs` 的模块头注释里（逐条对照
Android `KeyRouter.kt`）：空格 = 缓冲非空时提交、空缓冲时把空格直传给客户端；回车 = 缓冲非空时选中并提交第 1 个
候选、空缓冲时直通；退格 = 缓冲非空时按码点删、空缓冲时直通；有候选时 `1`–`9` 按**页内**
索引选词；`PageUp`/`PageDown` 翻页；**`Ctrl`/`Alt` 组合一律直通**（系统快捷键不拦）。

> ⚠️ 这几个键是**逐个枚举**过 fcitx5 库里的占用后选的，不是随手挑的。比如 `Ctrl+;`
> 看着更自然，但实测被剪贴板模块（`libclipboard.so`）抢走，且它是 PreInputMethod 阶段
> 的 watcher，拦不住。理由与占用表在 `opi_fcitx5.cpp` 的注释里，**改键位前先读那段**。
>
> ⚠️ **模式热键的实现目前在 C++ 侧，而 `input_method.rs` 的模块注释把 Ctrl/Alt 写成
> 「一律直通」** —— 两处读起来像矛盾，实际是热键在送进 Rust 路由**之前**就被 C++ 拦掉
> 了。这个归属是已知的临时状态，`opi_fcitx5.cpp` 里写明了长期归属是 Rust 侧。

### 4.4 中文标点 / 全角

引擎侧**有**这两个开关（`set_chinese_punct` / `toggle_chinese_punct` 等出口都在），
但 **Linux 端没有给它们配键位** —— 目前唯一的调用方是 Android 的 JNI 冒烟表。本插件
里只有 `Shift+Space` 那个全角**模式**切换。**别把它读成「按了没反应是 bug」。**

---

## 5. 词库

### 5.1 不需要你生成

已入库的那份就是 CMake 的缺省源（`android/app/src/main/assets/luna.opid`，与 Android
端发的是同一份文件）。**开箱即用**，不联网。

> 说「同一份」不只是句好话 —— 复核：
> `md5sum android/app/src/main/assets/luna.opid data/generated/luna.opid`
> （`data/generated/luna.opid` 是本地重编产物、**未入库**，见该目录的 `.gitignore`，
> 干净 checkout 上不存在 —— 所以别把构建指到那份。）

### 5.2 什么时候才需要生成脚本

只有你想**换词源**（改上游 pin、改排序规则）时：

```bash
python3 scripts/gen_luna_dict.py     # → data/generated/luna.opid（未入库）
```

⚠️ 这个脚本**要联网**（从 `raw.githubusercontent.com` 拉上游表，见脚本头部注释）。
生成后这样用它：

```bash
cmake -S crates/fcitx5-opi/cpp -B build-fcitx5 \
      -DOPI_LUNA_OPID=data/generated/luna.opid
```

### 5.3 繁体库：Linux 端装不了

`data/generated/trad.opid` 存在，但 **CMake 只装 `luna.opid`** —— Linux 端没有繁体
入口。引擎与数据侧的繁体能力是完整的，缺的是这一端的接线。

---

## 6. 许可证：包内必须带文本

代码是 MIT，但**这个包同时分发词库数据，而数据与代码分开授权**（`luna.opid` 由
LGPL-3.0 的 rime-luna-pinyin 与 Apache-2.0 的 rime-pinyin-simp 数据编译而来）。
`scripts/nfpm.yaml` 因此显式把两个文件装进 `/usr/share/doc/opi/`：

- `LICENSE`（代码，MIT）
- `data/raw/LICENSES.md`（数据，逐条来源与上游 pin）

⚠️ **少了这两个文件，包在授权上就是不完整的 —— 而代码照样能跑，所以它不会自己暴露。**
`nfpm.yaml` 的 `license:` 字段只够表达代码那一半，别把它读成整包的授权声明。

---

## 7. 已知限制（诚实清单）

- **openEuler 不可用**：该发行版没有 fcitx5 包（§2.1），中文输入走 ibus。
- **繁体库没有入口**（§5.3）。
- **中文标点 / 全角没有键位**（§4.4）。
- **学习结果不落盘**：`fcitx5-opi` 学完照旧丢（Rust 侧的学习落盘
  `engine-data/src/user_words.rs` 全仓零调用方）。
- **无障碍未接入**：读屏软件支持只有 Android 有（`README.md` 的平台状态表）。

## 8. 未验证清单（照抄现状，别读成已完成）

| 项 | 为什么没验 |
|---|---|
| **BlackArch / Debian sid 上的构建**【未验证】 | 本机没有镜像没跑过。同族的 **kali-rolling 已实测编得过**（§9），但别把同族当同结论 |
| **parrot rolling / mint 22 的实际安装**【推断，非实测】 | mint 镜像连拉多次失败（超时）；那两格的版本号是推出来的，**没有人在这两个发行版上装过** |
| **UOS / 银河麒麟**（含 `apt` 包名与自带 fcitx5 版本）【未验证】 | 需要授权镜像，本项目没有 |
| **真桌面前端下的实际输入**【未验证】 | §9 里那几格「装 + 加载 + 出字」是**容器里的无头 fcitx5**（真守护进程 + 私有 dbus 会话，走的是 addon 通路）—— **真 GTK/Qt 应用经 IM 模块收字的场景没人跑过**，X11 / Wayland 两种会话都没有 |
| **`fcitx5-remote -r` 能否替代 `fcitx5 -r -d`** | 本文只写实测过的那条；另一个没验，所以没写进来 |

## 9. 发行版矩阵

**逐格标证据档位** —— 「实测」= 有人真跑过，「推断」= 从别处推的、**没有跑过**。
两种别混着读。

| 发行版 | 包格式 | fcitx5 版本 | 状态 | 证据 |
|---|---|---|---|---|
| ubuntu 24.04 | deb | 5.1.7-1build3 | **装 + 加载 + 出字 `你好`** | 实测 |
| ubuntu 22.04 | deb | 5.0.14-1 | 同上 | 实测 |
| debian 12 | deb | 5.0.21-3 | 同上 | 实测 |
| debian 13 | deb | 5.1.12-2 | 同上 | 实测 |
| kali-rolling 2026.3 | deb | 5.1.21-1 | 同上；**且源码在 5.1.21 的头上编得过**（两个 target，零诊断） | 实测 |
| deepin 25（本机） | deb | 5.1.12 | 构建 + harness 全过 | 实测 |
| fedora 44 | rpm | 5.1.22 | 装 + 加载 + 出字 | 实测 |
| openSUSE Leap 16 | rpm | 5.1.13 | 同 Fedora | 实测 |
| arch | **不提供 .deb / .rpm** | 5.1.23 | 源码构建过 | 实测 |
| openEuler 24.03 | rpm | **不存在该包** | **不可用** —— 中文输入走 ibus（§2.1） | 实测 |
| parrot rolling | deb | 5.1.12-2 | 未装未测 | **推断**（官方包索引） |
| mint 22 | deb | 5.1.7 | 未装未测 | **推断**（两跳：与 ubuntu 24.04 同系） |
| UOS / 银河麒麟 | deb | — | 未实测（需授权镜像） | — |

> **这张表里哪部分是我自己量的**（免得你把它整体当成同一档证据）：
> `fcitx5` 版本号那一列，我在本地 docker 镜像里独立量过 debian 12/13、ubuntu 22.04/24.04、
> fedora、openSUSE Leap、arch、openEuler（无包）八格，与调研线的数逐格一致。
> **kali / parrot / mint 三格我没有独立复现**，版本号来自本轮并行的那条发行版调研。
> 「装 + 加载 + 出字」那一档的部署动作全部由调研线在容器里跑的。
>
> **kali 的「编得过」值得单说**：C++ 标准 17→20 那次修改（`d5c4d2b`）之前它编不过 ——
> 那条判断**已过期**，别再从旧材料里抄。

**要查你自己那一格**（比表里任何数字都准）：

```bash
fcitx5 --version                                   # 已装
apt-cache policy fcitx5                            # Debian 系仓库里有什么
pacman -Si fcitx5; dnf list --available fcitx5; zypper search -s fcitx5
```

## 10. 出问题时怎么定位

三个症状，三个不同的落点。**先分清是哪一种**，它们的修法完全不同：

| 症状 | 最可能的原因 | 查什么 |
|---|---|---|
| 输入法列表里**根本没有 OPI** | `inputmethod/opi.conf` 没装上 | 文件在不在 `/usr/share/fcitx5/inputmethod/`；**缺了它输入法就注册不出来**（可用列表是扫这个目录生成的，与 addon 的 `OnDemand` 无关） |
| **能选中 OPI，一个字都不出** | addon 整个没加载 ⇒ 九成是 fcitx5 版本低于门槛 | `fcitx5 --version` 对着 §1.1；这是**静默**的那种坏，日志里没有 opi 报错 |

⚠️ 第二条有个前提值得知道：**走包安装（§3.1）时你到不了这个症状** —— 包里的依赖带
版本约束，fcitx5 低于门槛时 `apt`/`rpm` 当场就拒了。会撞上它的只有**源码安装**
（§2.2 的 `cmake --install`）—— 那条路不过包管理器，没有任何东西替你把门槛拦在前面。
| 能打字，但**词少得可怜** | `luna.opid` 没装到 `<prefix>/share/opi/` | 文件在不在 `/usr/share/opi/`；装错目录时也是静默的 |

```bash
fcitx5-diagnose                    # 一把梭：版本、addon 目录、前端、环境变量
```

改落点后**别只跑 `cmake --install` 就算完** —— `build-fcitx5/opi_locate_check` 才是那个
「安装器说它装到了哪」==「fcitx5 说它去哪找」的对账点。CI 就是拿它当判据的。
