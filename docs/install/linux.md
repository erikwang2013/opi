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
| `/usr/share/opi/luna.opid` | 简体词库 | `Type::Data + "opi/luna.opid"` |
| `/usr/share/opi/trad.opid` | 繁体词库（2026-10-09 起） | `Type::Data + "opi/trad.opid"` |
| `/usr/share/opi/symbol_categories.tsv` | 符号大全类目数据（22 类目；与 Android 资产同源同字节） | `Type::Data + "opi/symbol_categories.tsv"` |
| `/usr/bin/opi-ctl` | 状态栏按钮的 CLI（D-Bus 打到插件，见 §4.8） | 状态栏悬浮条（按名字在 PATH 找） |
| `/etc/xdg/autostart/opi-statusbar.desktop` | 状态栏悬浮条随会话自启（2026-10-10 起） | XDG autostart |
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

⚠️ **`-r` 会先杀掉正在跑的那个实例。** 万一新的没起来，你面对的是**所有输入法都不
可用**（看着像「我的输入法被删了」，其实包和配置一个字没动 —— §10 最后一行）。兜底：
**登出再登入**，登录时 fcitx5 由 `/etc/xdg/autostart/org.fcitx.Fcitx5.desktop` 拉起，
环境是齐的。

### 4.2 把 OPI 加进输入法组（**不做这步，打字什么都不会发生**）

**新装的输入法不会自动启用。** 装完 `.deb` / `.rpm` 之后，OPI 会出现在「可用输入法」
的候选列表里，但**不在输入法组里** —— 这一步不做，**输入法列表里看得到 OPI、打字却
什么都不会发生**。用户侧它长得像「安装无效」，其实包是好的。

打开「输入法配置」窗口有两条路，通向同一个窗口：

| 入口 | 怎么做 |
|---|---|
| 图形界面 | 右键点**托盘里的 fcitx5 图标** → 「配置」 |
| 命令行 | 运行 `fcitx5-configtool` |

> 图形那条写的是 fcitx5 的常见布局：**本机没有图形会话，未在本机 GUI 验证**【未验证】。
>
> 命令行那条【实测】：本机 `which fcitx5-configtool` → `/usr/bin/fcitx5-configtool`；
> 本机（Deepin 25，`fcitx5` `5.1.12-2deepin12`）`dpkg -S` 显示这个文件**由 `fcitx5` 包
> 自己提供**，而本仓库的包依赖 `fcitx5`（`scripts/nfpm.yaml` 的 `overrides`），所以装完
> 本包就应该有它。**其它发行版没核过** —— 报 `command not found` 时，先去找你那个发行版
> 的 fcitx5 配置工具包。

窗口里怎么点（这几步是 **UI 描述，未在本机 GUI 验证**【未验证】）：

1. 切到 **「输入法」** 页
2. 这一页有**两块列表**：一块是**当前的输入法组**（打字时真正生效的清单），另一块是
   可添加的**候选列表**
3. 在候选列表里找 **OPI**
4. 选中它 → **加进输入法组** → 点 **「应用」**

#### 命令行加进去（**推荐甲：不用停 fcitx5，也不动别的输入法**）

上面那几步没在本机 GUI 上验证过，所以这里另给命令行路子。**有两条，优先用甲。**

##### 甲、走 D-Bus（推荐）

fcitx5 有一组**配置用**的 D-Bus API —— 图形界面的「输入法配置」窗口走的就是它。
用它改的是**内存里的输入法组**，由 fcitx5 自己落盘，所以根本不存在「手改文件被实例
写回覆盖」那回事：**不用停 fcitx5，你的搜狗（或任何别的输入法）全程可用。**

```bash
# 1) 先看当前组长什么样（只读，不改任何东西）
#    ⚠️ 组名（本机是「默认」）先查出来，别照抄 —— 你那儿可能叫别的
busctl --user call org.fcitx.Fcitx5 /controller \
  org.fcitx.Fcitx.Controller1 InputMethodGroups
busctl --user call org.fcitx.Fcitx5 /controller \
  org.fcitx.Fcitx.Controller1 InputMethodGroupInfo s 默认

# 2) 把 opi 追加进去 —— 已有项原样保留，顺序与 DefaultIM 都不动
busctl --user call org.fcitx.Fcitx5 /controller \
  org.fcitx.Fcitx.Controller1 SetInputMethodGroupInfo 'ssa(ss)' 默认 us 3 \
  "keyboard-us" "" "com.sogou.ime.ng.fcitx5.deepin" "" "opi" ""

# 3) 落盘
busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 Save
```

> ⚠️ **第 2 条是照抄本机的，你必须改两处**：
> ① `com.sogou.ime.ng.fcitx5.deepin` 是**本机搜狗**的名字 —— 换成第 1 条打印出来的
> **你自己的项**；② 那个 `3` 是你**总共有几项**（本例 = 原有 2 项 + opi）。数错
> `busctl` 会报参数不匹配，**不会写坏东西**。格式是成对的「输入法名 + 布局」，
> 最后一项固定是 `opi` 配空布局。

**切过去用** —— 这一步不只是「用一下」，**它才是触发插件加载的动作**（见下）：

```bash
busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 SetCurrentIM s opi
```

**验收判据**：切到 opi 后敲 `nihao` + 空格 → **应该上屏「你好」**。

**反悔**（退回原样，不留痕迹）：把第 2 条换成**第 1 条原本打印的那串**，再 `Save` 一次。

> 【实测：本机 Deepin 25、用 1.3.4 的**已安装载荷**。全程**没停过 fcitx5**、没碰
> profile 文件；搜狗始终是 `DefaultIM` 且可正常输入；切到 opi 后插件加载了 **9 条
> 映射**（`/proc/<pid>/maps` 里的 `libfcitx5_opi.so` + `libfcitx5_opi_glue.so`）。】

⚠️ **加载的真正触发条件是「在组里 + 切到它」** —— 不是 `--enable`，也不是重启。
实测：加进组但**没切过去** → `maps` 命中 **0**；切过去 → **9**。**只加不切等于没装。**

⚠️ **查加载时挑 pid 别用 `pgrep -x fcitx5 | head -1`** —— 它连僵尸一起匹配（本机现有
14 个 `[fcitx5] <defunct>`），挑到僵尸会让 `maps` 读成 0 条，得到一个**假的「没加载」**：

```bash
p=$(pgrep -x fcitx5 | while read x; do [ "$(awk '{print $3}' /proc/$x/stat)" != Z ] && echo $x; done | head -1)
grep -c 'libfcitx5_opi' /proc/$p/maps
```

##### 乙、手改 profile（**最后手段**：它会把你的输入法停掉）

只有**没有会话 D-Bus**（纯 TTY / 远程 / headless）时才走这条。代价如实说：**它要把
fcitx5 停掉、改完再起** —— 中途**你所有输入法都用不了**；哪一步没成，你面对的就是
「输入法整个没了」，看着像「我的搜狗被删了」，其实配置一个字没动。

> ⚠️ **先改 profile 再重启 = 白改。** 运行中的实例退出时会**把它内存里的 profile
> 写回磁盘**，覆盖你手改的那一段。
>
> 【实测：加进去的 `[Groups/0/Items/2]` 事后**从文件里消失了**，文件被重写成带
> `# Group Name` / `# Layout` 注释的样子 —— 那是 fcitx5 **自己的序列化器**写的。】
>
> ⇒ 顺序必须是**先停、再改、再起**；**先改后停，改动必丢**。

```bash
# 1) 先停
fcitx5-remote -e

# 2) 确认真的停了 —— 退出码 0 = 还活着（先别改），1 = 已经停了（可以改）
ps -C fcitx5 -o stat= | grep -qv '^Z'

# 3) 这时才改 ~/.config/fcitx5/profile：
#    在 [GroupOrder] 那一行**之前**插入下面三行
#      [Groups/0/Items/2]
#      Name=opi
#      Layout=
#    ⚠️ 编号必须是组里**空着**的号。撞上已有编号会**顶掉那一项**（可能正是你在用的
#    输入法）—— 先数清组里有几项，用下一个号。

# 4) 再起 —— 这一步没成你就没有输入法了，起完务必确认
fcitx5 -d

# 5) 切过去（同甲：这一步才触发加载）
fcitx5-remote -s opi
```

> **恢复**：`fcitx5 -d` 起不来时，**先把 profile 改回原样**再起，或**登出再登入**
> （登录时 fcitx5 由 `/etc/xdg/autostart/org.fcitx.Fcitx5.desktop` 拉起，环境是齐的）。
>
> ⚠️ **第 2 步为什么不能用 `pgrep -x fcitx5`**（两个方向都会骗你，本机实测）：
>
> - **假阳**：它连**僵尸进程**一起匹配。本机就有 **14 个** `[fcitx5] <defunct>`，
>   于是它**永远有输出** —— 你按「无输出才算停」判，会**卡在第 2 步出不来**。
>   （`Z` = 已死透、地址空间都没了，退出中的实例**不可能**再回写你的 profile。）
> - **`pgrep -x fcitx5 -r running` 假阴**：`-r running` 只匹配 `R`（正在 CPU 上跑），
>   而**闲着等事件的 fcitx5 是 `S`** —— 它对一个**活得好好的**实例报「没有」，你会
>   以为停了、去改，然后**正好撞上那个覆盖陷阱**。
>   【阳性对照：`ps -C sleep -o stat=` 显示 7 个活进程都是 `Ss`，
>   `pgrep -x sleep -r running` 对它们**全报 0 命中**。】
>
> 上面那条 `ps ... | grep -qv '^Z'` 两个对照都过了：活 `sleep` → 退出码 0；
> 全僵尸 `fcitx5` → 退出码 1。
>
> ⚠️ **`fcitx5-remote -r` 不能替代第 1 步** —— 实测只做它的话，加载次数**仍然是 0**。
> 顺带记住 `fcitx5-remote -r` 与 `fcitx5 -r`（= `--replace`）**不是一回事**：
> 前者是 remote 协议里的一条，后者是进程级替换（§4.1 用的那个）。

⚠️ **是「往组里加一项」，不是「把原来那项换成 OPI」。** `DefaultIM` 保持原样 ——
本机实测时它仍是搜狗，所以在用户主动切过去之前，**行为一切不变**。把原来那项删掉
等于拿掉一个本来能用的输入法。加完之后组里**至少要有两项**（例如原来的 + `opi`）。

> **找不到 OPI？** 先看下面两条，别先怀疑包：
>
> 1. **按语言过滤被藏起来了** —— 那个「只显示当前语言」的过滤项是常见嫌疑。
>    本插件把这条输入法声明的语言写成 `zh_CN`、中文界面下的名字是「opi输入法」
>    （候选列表里显示 `Name[zh_CN]`，加进组之后显示 `Label`）—— **【推断：读
>    `crates/fcitx5-opi/data/inputmethod/opi.conf` 的 `Name` / `Name[zh_CN]` / `Label` /
>    `LangCode`，没有在本机 GUI 上看过】**。所以过滤项停在中文以外的语言时，它会被藏起来。
> 2. **没做 §4.1 的那次重启** —— ⚠️ 但**重启只是必要条件，不是充分条件**：真正让
>    插件加载的是「**opi 在输入法组里 + 切到它**」（§4.2 命令行那节的单变量对照）。
>    重启完仍然不出字，别以为「重启过了所以肯定加载了」—— 那是本次最常见的误判。
>    【未验证：装完但**没**重启时，候选列表刷不刷新。本文没测过这一格。】
>
> 两条都排除了还是看不到，按 §10 的第一行先查文件在不在。

### 4.3 键位

模式热键判在 Rust 路由**之前**（`crates/fcitx5-opi/cpp/opi_mode.h` 的
`handleModeHotkey` / `opi_fcitx5.cpp` 的 `handleFullwidthHotkey`）。四个键都在
**同一个模式上来回切**，不是单向进入：

| 键 | 作用 |
|---|---|
| `Ctrl+'` | 英文 ⇄ 拼音 |
| `Ctrl+\` | 符号 ⇄ 拼音 |
| `Ctrl+Shift+F` | 简体 ⇄ 繁体（2026-10-09 起；繁体查 `trad.opid`，见 §5.3） |
| `Shift+Space` | 全角 ⇄ 半角 |
| `Ctrl+Shift+Z` | 符号大全（2026-10-10 起；⚠️ 与编辑器「重做」冲突是已知项，见 §4.8） |

> `Ctrl+Shift+F` 只认**大写的 F**（真前端对 Shift+字母给的就是大写 keysym）——
> 单按 `Ctrl+F`（应用们的「查找」）**不会**被吃掉。注意实现是按 `rawKey()` 判的：
> `key()` 被 `normalize()` 折过（任何修饰键 + 小写字母一律折成大写），
> Ctrl+F 与 Ctrl+Shift+F 在那里分不开 —— 这是实测踩出来的（细节在 `opi_mode.h`）。

其余按键路由在 `crates/fcitx5-opi/src/input_method.rs` 的模块头注释里（逐条对照
Android `KeyRouter.kt`）：空格 = 缓冲非空时提交、空缓冲时把空格直传给客户端；回车 = 缓冲非空时选中并提交第 1 个
候选、空缓冲时直通；退格 = 缓冲非空时按码点删、空缓冲时直通；有候选时 `1`–`9` 按**页内**
索引选词；`PageUp`/`PageDown` 翻页，**组字中 `-` / `=` 也翻页**（`-` 上一页、`=` 下一页；
2026-10-09 用户报障补上，空缓冲时不抢键）；**`Ctrl`/`Alt` 组合一律直通**（系统快捷键不拦）。

> ⚠️ 这几个键是**逐个枚举**过 fcitx5 库里的占用后选的，不是随手挑的。比如 `Ctrl+;`
> 看着更自然，但实测被剪贴板模块（`libclipboard.so`）抢走，且它是 PreInputMethod 阶段
> 的 watcher，拦不住。理由与占用表在 `opi_mode.h` 的 `handleModeHotkey` 注释里
> （2026-10-09 从 `opi_fcitx5.cpp` 搬过去），**改键位前先读那段**。
>
> ⚠️ **模式热键的实现目前在 C++ 侧，而 `input_method.rs` 的模块注释把 Ctrl/Alt 写成
> 「一律直通」** —— 两处读起来像矛盾，实际是热键在送进 Rust 路由**之前**就被 C++ 拦掉
> 了。这个归属是已知的临时状态，`opi_mode.h` 里写明了长期归属是 Rust 侧。

### 4.4 中文标点 / 全角

引擎侧**有**这两个开关（`set_chinese_punct` / `toggle_chinese_punct` 等出口都在），
但 **Linux 端没有给它们配键位** —— 2026-10-08 起它们改由**属性设置**（见 §4.6）控制：
中文标点 = 设置里的一个开关，全角仍是 `Shift+Space` 那个**模式**切换（两者相互独立，
见 `engine.rs` 的 `punct_text` 真值表）。**键盘上没有这两个键位，别读成「按了没反应是 bug」。**

### 4.5 候选序号（「1. 你」）

候选前的序号**由插件显式声明**，不是 fcitx5 给的默认值。判据是探针（真库、**未起守护进程**）：

| `CommonCandidateList` 的状态 | `label(0..2)` |
|---|---|
| 什么都不调（2026-10-08 之前的 OPI） | `''` `''` `''` —— 面板一个序号都不画 |
| `setSelectionKey({"1",…})` 之后（现在的做法） | `'1. '` `'2. '` `'3. '` |

`opi_fcitx5.cpp` 的 `refreshingPanelFor()` 因此给候选表设了**选择键**，键数按实际候选数给：
序号是**页内**编号（1–8，翻页后从 1 重数），与 Android / 鸿蒙 / Windows 候选窗同义；
顺带把数字键的选择路由交给 fcitx5 的候选机制（落点同样是 `OpiCandidateWord::select`
的页内索引）。门禁：`opi_panel_driver` 的第 2 节会逐条核对 `label(i)` 以页内数字开头。

⚠️ 面板**画不画**这个序号、画成什么样（字号/间距）由主题与 classicui 决定 —— 本层只保证
「label 不再是空的」。看不到序号时先看主题那侧，别改这里。

### 4.6 属性设置（六个选项；入口：配置面 + 状态栏 + 桌面窗）

**配置落在 fcitx5 自己的 addon 配置里**：`$XDG_CONFIG_HOME/fcitx5/conf/opi_fcitx5.conf`
（字段声明 = `crates/fcitx5-opi/cpp/opi_config.h` 的 `OpiSettings`）：

```ini
Learn=True          # 学习（记住选过的词）
ChinesePunct=True   # 中文标点（，。、与成对引号）
Fuzzy=True          # 模糊音（z/zh、n/l…）
Jianpin=True        # 简拼（声母缩写，nh → 你好）
ShiftToggle=True    # Shift 轻按切换中英文（2026-10-09 起）
Skin=不改动          # 候选面板皮肤（2026-10-09 起；值存的就是「不改动/浅色/深色/宣纸/朱砂」）
```

**入口在配置工具里**（2026-10-09 用户裁决「设置功能不能在外层设置，必须到配置里才有设置入口」）：

- Deepin 控制中心 → 键盘和语言 → **输入法**（页面名「输入法管理」）→ **高级设置 →
  附加组件 → OPI** →（鼠标悬停时出现的）**齿轮** → 逐项开关/下拉。上游 fcitx5-configtool
  同位置同交互（措辞随版本略有出入）。
- 机制（本机核实，非推导）：配置工具按 `fcitx://config/addon/opi_fcitx5` 向守护进程要
  **值 + 描述**（每个选项带中文说明）再逐项渲染。实测 `busctl call org.fcitx.Fcitx5
  /controller org.fcitx.Fcitx.Controller1 GetConfig s 'fcitx://config/addon/opi_fcitx5'`
  六项齐全。
- ⚠️ **图形设置窗进不了「配置」这一层**：fcitx5 的 `ConfigureAddon` 是 **no-op**
  （5.1.12 上游源码：`void Instance::configureAddon(const std::string &) {}`，
  D-Bus 侧同款转调 `configure()` = 只启动配置工具本身）—— 框架**没有**从配置面拉起
  外部程序的通道。所以设置入口是**两层**的：配置面里是上面这些原生选项；图形设置窗
  走系统菜单入口（见 §4.7）。

**状态栏（外层）三个入口**（2026-10-09 用户裁决「外层……没有设置入口」「没有切换繁体」
补齐；实现在 `opi_mode.h` 的 `OpiShiftToggle`/`OpiFanjianToggle` 与 `opi_status.h`）：

| 项 | 点击效果 |
|---|---|
| `中` / `英` | 中英文切换（与 Shift 轻按同一条 toggle） |
| `简` / `繁` | 简体 ⇄ 繁体（与 `Ctrl+Shift+F` 共用同一个实现 `opiToggleTraditional`） |
| `设` | 打开属性设置窗 |

- 三项都挂在 fcitx5 的 `StatusGroup::InputMethod` 状态区，随输入法激活挂上。
- 两个模式项的文案显示**当前**状态，且**互相同步**：从英文态点「简」会切进繁体，
  「中/英」项同时刷回「中」（`setOnModeChanged` 钩子，见 `opi_engine.h` 构造）。
- `设` 的启动顺序：`/opt/opi-candidates/bin/opi-candidates --settings`（合并包落点）→
  PATH 里的 `opi-candidates` → `fcitx5-configtool` 兜底（至少能到配置面）。
  ⚠️ 如实记：它只保证**发起启动**（`posix_spawnp`），窗口有没有起来这里看不到；
  一个都找不到时往 stderr 打一行。

**改它有三条路，都即时生效**（addon 的 `reloadConfig()` → 引擎，不用重启输入法）：

1. 配置工具里那个齿轮（保存即生效）；2. 手改文件 + **`busctl --user call org.fcitx.Fcitx5
/controller org.fcitx.Fcitx.Controller1 ReloadAddonConfig s opi_fcitx5`**；
3. `opi-candidates --settings`（本仓库的 Compose 设置窗；它**保留未知键**合并写，
不会抹掉自己不认识的选项，保存后走的就是第 2 条那个调用）。

> ⚠️ **订正（2026-10-09 实测）**：此前此处写的是「手改文件 + `fcitx5-remote -r`」——
> **那条不生效**。现场：文件里 `Skin=宣纸`、守护进程里还是 `Skin=深色`，
> `fcitx5-remote -r`（= D-Bus `ReloadConfig`）反复调用、`GetConfig` 纹丝不动；
> 换 `ReloadAddonConfig("opi_fcitx5")` 立刻回新值（皮肤也跟着换）。
> 设置窗已改为三级兜底调后者（busctl → dbus-send → 旧路 `fcitx5-remote -r`）
> —— 报障「桌面端设置不能直接生效」即此。

**Shift 切换中英文（ShiftToggle，默认开）**：轻按**左或右** Shift 在 中文 ⇄ 英文 间切换
（搜狗同款手势）。

- 「轻按」= 按下后抬起前**没有别的键按下**（Shift+字母/方向/空格都不算），且按住时长
  不超过 `ModifierOnlyKeyTimeout`（fcitx5 全局配置里的那个值，缺省 250ms；复用框架
  同一个判定 helper）。
- 英文模式 = 引擎既有的 English 模式（与 `Ctrl+'` 同一条通路）：字母直传上屏、中文标点
  停用。切入时会顺带清掉「单次 Shift」状态 —— 否则切换后的第一个字母会莫名大写。
- ⚠️ 别把 Shift 再配到 fcitx5 的**全局触发键** `TriggerKeys`：那是框架级的
  激活/非激活切换（**所有**输入法都受影响，且组里多于两个输入法时是轮换语义）。
  Deepin 控制中心「输入法 → 快捷键 → 中英文切换 = Shift」改的就是那个 —— 两种机制
  二选一，同时开会双重切换。
- ⚠️ fcitx5 < 5.1.12 上没有超时判定（那版没有这个 API，代码按能力探测降级）：按住
  很久再抬起也会切一次；组合键的撤销判定不受影响。

**皮肤（Skin）**：内置四张 —— 浅色 / 深色 / 宣纸 / 朱砂。⚠️ **候选面板不是本插件画的**：
它由 UI addon（本机是 classicui）按主题绘制，所以「换皮肤」= 改
`~/.config/fcitx5/conf/classicui.conf` 的 `Theme=`/`DarkTheme=` 并热重载 classicui。

- **对所有输入法生效**（面板主题是框架级的一份设置，做不到只影响 OPI，如实记）。
- 「不改动」= OPI 不介入（默认）。选皮肤只在**值发生变化**时才写文件：fcitx5 每次启动
  不会拿旧值把用户后来改的主题顶回去。
- **图形化的选法**（2026-10-09 起）：设置窗（§4.7，菜单「opi输入法」或状态栏「设」）
  有「外观」一节 —— 五张**预览卡**（底色 + 样字 + 强调条），点击即换。颜色取自
  `scripts/gen_fcitx5_themes.py` 的调色板，与配置面 `Skin=` 的五个字面值一一对应。
- 想换回系统主题：同一张「附加组件」列表里找**经典界面**（classicui）→ 齿轮 → 主题改回
  `deepin-light`/`deepin-dark`；或把本项选回「不改动」后自己改。
- ⚠️ 面板由 **classicui** 绘制才看得见这套主题（fcitx5-qt 等别的 UI 不吃它；
  那种环境下文件仍会写、面板不变 —— 重载对未加载的 addon 是 no-op，不会多起面板）。
- 主题内容是生成产物：`python3 scripts/gen_fcitx5_themes.py`（`--check` 与产物对账）。

⚠️ 读侧**容忍**：值大小写不敏感、`#` 行内注释、未知键忽略、缺键回默认（默认**四项全开 +
ShiftToggle 开 + 皮肤「不改动」**= 引擎既有行为/不介入）。手改坏一个值只会让那一项回默认。

### 4.7 桌面应用（设置窗/候选窗）与打包

`desktop/` 的 Compose 应用：Windows 上 = 候选窗（TSF 命名管道两端都在那边），
Linux 上 = `--settings` 的设置窗（无参数启动也是它）。**v1.6.0 起它与插件装在同一个包里**
（`.deb`/`.rpm` 一手装齐 —— `scripts/build-packages.sh` 的 `OPI_DESKTOP_DEB`；包体因此从
~1.7MB 到 ~48MB，jpackage 自带 JRE，这是裁决时摆过的数）。

- **系统菜单里有入口**：`/usr/share/applications/opi-settings.desktop`（`Name=opi输入法`，
  点开即设置窗）—— 2026-10-09 晚用户裁决「设置功能要在外层可以设置」，由打包后注入
  生成（理由与核对法见 `desktop/build.gradle.kts` 文件尾）。另有两条直启路：
  `opi-candidates --settings`（装出来在 `/opt/opi-candidates/bin/opi-candidates`；
  状态栏悬浮条是同一二进制的 `--statusbar` 模式，见 §4.8）、
  以及配置面的原生选项（§4.6）—— **两层都能设置**；状态栏还有第三个入口
  （「设」，见 §4.6）。
- 设置窗内容：常规（学习 / 中文标点）、输入（模糊音 / 简拼）、**外观**（皮肤预览卡，
  Linux 才有；Windows 不显示 —— 那边没有 fcitx5 主题这回事）、**状态栏**（悬浮条开关 +
  逐按钮常驻，见 §4.8）、关于。
- 版本号是**单一来源**：`desktop/build.gradle.kts` 的 `opiAppVersion` 同时喂给
  jpackage 包版本与窗口标题（构建时生成 `Version.kt`）。此前窗口里写死过一份，
  到 1.6.0 就成了摆在用户眼前的假话 —— 别再写死第二处。
- 升级路径：装新包时 apt/dnf 按 `replaces`/`conflicts` 自动卸掉旧的独立设置窗包
  `opi-candidates`（≤1.5.0）—— 不声明的话 `/opt/opi-candidates/**` 文件归属冲突会让
  dpkg **拒绝安装**。

### 4.8 状态栏悬浮条 / 符号大全 / 虚拟键盘（2026-10-10）

**状态栏悬浮条**（自绘；2026-10-10 用户裁决「直接上自绘悬浮条」）：

- 8 个图标按钮：软键盘 / 符号大全 / 简繁转换 / 中英文符号切换 / 中英文切换 /
  logo（拖动把手，单击开设置）/ 全半角切换 / 设置。
- 自启：`/etc/xdg/autostart/opi-statusbar.desktop`（装包即生效，登录自动起）；
  手动起：`/opt/opi-candidates/bin/opi-candidates --statusbar`。
- 显示/隐藏：设置窗「状态栏」一节（master 隐藏 + 逐按钮常驻）。配置与 §4.6 的六个选项
  **同住一个文件**（`$XDG_CONFIG_HOME/fcitx5/conf/opi_fcitx5.conf`，九键
  `StatusBarHidden` / `StatusBar*`；写入在 `desktop/` 的 `Settings.kt`），改完 1s 内生效。
  ⚠️ fcitx5 配置工具保存时会不会把这九键抹掉：**未取证**（配置面只声明它认识的六个选项），
  已在 `Settings.kt` 头注释标注。
- 按钮动作：「符号 / 简繁 / 标点 / 全角」走 `opi-ctl`（下节）；「中英文切换」切的是
  **引擎英文模式**（与 `Ctrl+'`/Shift 轻按同一条 toggle）—— 不是停用整个输入法
  （那样 OPI 热键会全失效）。
- ⚠️ 与 fcitx5 自己的状态区（§4.6 的「中/英/简/繁/设」三项）**两套并存**：那套挂在
  面板上，这套是独立悬浮窗；两边调同一批引擎动作，状态互相同步。

**符号大全**（`Ctrl+Shift+Z`，或状态栏「符号」按钮 / `opi-ctl symbols`）：

- 走 fcitx5 **原生候选面板**：一级 22 类目（`1.常用` … `22.藏文`），数字进类目；
  二级符号按页出，`-`/`=` 翻页、数字上屏（上屏后回一级）、`Esc` 退出（吞键）。
- 数据 `/usr/share/opi/symbol_categories.tsv`（真源 = `scripts/gen_symbol_categories.py`
  的双输出之一，另一份给 Android，两份逐字节相同）。缺失/坏文件 ⇒ 入口禁用 +
  `opi-ctl symbols` 返 4（「数据缺失」）—— 不会半开半死。
- ⚠️ `Ctrl+Shift+Z` 与编辑器「重做」冲突（搜狗同键；这是默认选择）：中文态下会被
  符号大全吃掉。要换键位：`opi_fcitx5.cpp` 里的单一常量点。
- 切英文/切 IM 时若正在浏览：先退浏览恢复真缓冲、再走「直接确认」（§4.3 的
  `Ctrl+'` 那条）—— 测试里钉了顺序，防止把「符号」标题当输入提交出去。

**虚拟键盘**（状态栏「软键盘」）：

- 无边框置顶键盘窗（qwerty + 数字行 + 底行 `[符][空格][回车][退格]`）。「符」是**动作键**
  （2026-10-10 用户裁决「要加」，对齐 Android 9 键底行惯例）：开符号大全，与状态栏
  「符号」按钮**共用同一条动作**（Linux 拉 fcitx5 候选面板 / Windows 开符号页）。
  其余键经 **XTEST** 注入系统
  —— 注入的键与物理键盘同路（照常经过 fcitx5：中文态下打字母出候选是**正确**行为）。
- ⚠️ **Wayland 原生窗口不支持**（XTEST 到不了）；XWayland 应用可用。X11 会话可用。

**`opi-ctl`**（状态栏与脚本共用的 CLI）：

```bash
opi-ctl symbols|trad|punct|fullwidth|lang
```

- 通道：session 总线上 `org.fcitx.Fcitx5` 的 `/opi`（接口 `org.fcitx.Fcitx5.Opi1`，
  插件内建服务）——作用于**当前输入上下文**，不是「改配置 + 重载」。
- 退出码：`0` 成功 / `2` 用法 / `3` 通道不通（fcitx5 没跑或缺 dbus 模块）/
  `4` 功能不可用（如符号数据缺失）/ `5` 动作被拒。状态栏按非零给可见反馈。
- ⚠️ 它与「设置窗保存」的重载路（`ReloadAddonConfig`，见 `Settings.kt` 的订正段）
  是两条不同的路：那条改配置，这条做动作。别互相替代。

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

### 5.3 繁体库：已接线（2026-10-09）

`trad.opid` 与 `luna.opid` 一样**随包安装**（`/usr/share/opi/trad.opid`，
CMake 的 `OPI_TRAD_OPID`，缺省取入库副本 `android/app/src/main/assets/trad.opid`）。
切换键是 **`Ctrl+Shift+F`**（§4.3），繁体模式下候选查的就是它。

- **可选增强语义**：trad 装坏了/缺失时**不报错**，Traditional 模式回退 luna
  （表现为「切了繁体、打出来还是简体」——那正是「回退」而不是「没装」的形态；
  引擎侧只有一行 stderr）。luna 坏才是硬错误（整个插件没词库）。
- 数据来源与授权：Unicode Unihan + rime-terra-pinyin（LGPL-3.0），生成器是
  `scripts/gen_trad_dict.py`（**不是** `gen_luna_dict.py`）——逐条见
  `data/raw/LICENSES.md`。
- 落点门禁：`opi_locate_check` 的第 4 个参数 + `ci.yml` 的「查找常量没漂」grep
  两条都盯着它（与 luna 同规格）。

---

## 6. 许可证：包内必须带文本

代码是 MIT，但**这个包同时分发词库数据，而数据与代码分开授权**（`luna.opid` 由
LGPL-3.0 的 rime-luna-pinyin 与 Apache-2.0 的 rime-pinyin-simp 数据编译而来；
`trad.opid` 由 Unicode Unihan 与 LGPL-3.0 的 rime-terra-pinyin 编译而来 ——
逐条来源见 `data/raw/LICENSES.md`）。
`scripts/nfpm.yaml` 因此显式把两个文件装进 `/usr/share/doc/opi/`：

- `LICENSE`（代码，MIT）
- `data/raw/LICENSES.md`（数据，逐条来源与上游 pin）

⚠️ **少了这两个文件，包在授权上就是不完整的 —— 而代码照样能跑，所以它不会自己暴露。**
`nfpm.yaml` 的 `license:` 字段只够表达代码那一半，别把它读成整包的授权声明。

---

## 7. 已知限制（诚实清单）

- **openEuler 不可用**：该发行版没有 fcitx5 包（§2.1），中文输入走 ibus。
- **繁体词库坏时静默回退简体**（§5.3 的可选增强语义 —— 只有一行 stderr，
  用户侧表现为「繁体模式打出来还是简体」）。
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
| **opi 在真桌面应用里的实际输入**【未验证】 | §9 那几格「装 + 加载 + 出字」是**容器里的无头 fcitx5**（真守护进程 + 私有 dbus 会话，走 addon 通路）。本机 Deepin 桌面上已证实 fcitx5 持有 X11 + treeland 连接（搜狗一直能正常输入），但**「切到 opi 后在 GTK/Qt 应用里敲出中文」这一步没人做过** —— 差的是 opi 自己的那一段，不是桌面通路那一段 |
| **「输入法配置」窗口里点的那几下**（§4.2 图形界面那几步）【未验证】 | 本机没有图形会话；深度的 `fcitx5-configtool` 会转交给 `org.deepin.dde.ControlCenter1`，没找到可驱动的窗口。§4.2 **甲（D-Bus）**走的是同一个 API、可作替代方案，但**替代 ≠ 验证过那个窗口** |
| **§4.2 甲（D-Bus）在别的发行版上**【未验证】 | 只在**本机 Deepin 25** 上实测过。`busctl` 与那两个方法名在别的 fcitx5 版本上没核过 |
| **`fcitx5-remote -r` 能否替代 `fcitx5 -r -d`**（§4.1 的用途）【未验证】 | 本文只写实测过的那条。**已实测的是另一个用途**：改完 profile 之后只做 `fcitx5-remote -r`，加载次数仍是 **0**（§4.2）—— 那是「改配置后重载」，与「装完让 addon 加载」不是同一个问题，别把这条结论搬过去 |
| **`Ctrl+Shift_L` 那个切换热键** | 实测的 harness 里合成事件到不了 core 的热键匹配（同一条 dbus 通路**能**把引擎级热键送到，所以指向 harness 限制，不指向桌面）。**不能据此说它坏了** |
| **真登出 / 再登录**（§4.1 的替代做法） | 只跑过 `fcitx5 -d` / `fcitx5 -r -d`；真会话的登出登录没人跑过 |

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
| deepin 25（本机） | deb | 5.1.12 | **装 + 加载 + 出字 `你好`**（1.3.4 的**已安装载荷** + harness）；**真桌面实例上也已加载**（§4.2 甲，9 条映射）—— 但**桌面应用里敲中文那步没做**（§8）；构建过 | 实测 |
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

**先分清是哪一种**，它们的修法完全不同：

| 症状 | 最可能的原因 | 查什么 |
|---|---|---|
| 输入法列表里**根本没有 OPI** | `inputmethod/opi.conf` 没装上 | 文件在不在 `/usr/share/fcitx5/inputmethod/`；**缺了它输入法就注册不出来**（可用列表是扫这个目录生成的，与 addon 的 `OnDemand` 无关） |
| 列表里有 OPI，**切过去一个字都不出** | ① **opi 不在输入法组里**（最常见，§4.2）；② addon 整个没加载 ⇒ fcitx5 版本低于门槛 | ① 看 `~/.config/fcitx5/profile` 的组里有没有 `Name=opi`；② `fcitx5 --version` 对着 §1.1。两种都是**静默**的那种坏，日志里都没有 opi 报错 |
| **手改过 profile、重启后还是不出字** | 改的时候实例还在跑 ⇒ **退出时把你的改动替回去了**（§4.2 乙） | 改走 §4.2 **甲（D-Bus）** —— 那条不用停 fcitx5，没有这个坑。若必须走乙：先 `fcitx5-remote -e`、用乙第 2 步那条判据确认它**真**停了、然后才改 |
| **所有输入法都没了，像被删了** | **fcitx5 根本没在跑** —— 包和配置多半一个字没动，别急着重装 | `busctl --user list \| grep -i fcitx`；有 `org.fcitx.Fcitx5` 就是在跑。**不在跑就登出再登入**（最稳，环境齐全），或按 §4.2 乙第 4 步起。⚠️ 起完还要确认它**连上了显示**，否则能应答 D-Bus 却收不到按键 |
| 能打字，但**词少得可怜** | `luna.opid` 没装到 `<prefix>/share/opi/` | 文件在不在 `/usr/share/opi/`；装错目录时也是静默的 |

⚠️ 第二行的②有个前提值得知道：**走包安装（§3.1）时你到不了它** —— 包里的依赖带
版本约束，fcitx5 低于门槛时 `apt`/`rpm` 当场就拒了。会撞上它的只有**源码安装**
（§2.2 的 `cmake --install`）—— 那条路不过包管理器，没有任何东西替你把门槛拦在前面。

```bash
fcitx5-diagnose                    # 一把梭：版本、addon 目录、前端、环境变量
```

⚠️ **`fcitx5-diagnose` 的 `Addon List:` 那一节会骗你** —— 它是**扫目录**：把
`/usr/share/fcitx5/addon/*.conf` 挨个读一遍 `Name`/`Version` 列出来，**从不真的
`dlopen`，也从不判 `[Addon/Dependencies]`**。所以**下面这个版本门槛它查不出来**：
fpv5 版本低于门槛时，`OPI` **照样出现在那张列表里**，看上去一切正常。
【实测：拿一个「addon 根本没加载」的系统跑它，输出与加载成功的那台**逐字相同**。】
⇒ **判「加载了没有」别看它**，看 §10 第二行那两条。

改落点后**别只跑 `cmake --install` 就算完** —— `build-fcitx5/opi_locate_check` 才是那个
「安装器说它装到了哪」==「fcitx5 说它去哪找」的对账点。CI 就是拿它当判据的。
