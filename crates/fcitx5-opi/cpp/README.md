# fcitx5 插件胶水（B2）

`opi_fcitx5.cpp`：fcitx5 AddonInstance + InputMethod 薄层，调用 Rust 逻辑
出口（`libfcitx5_opi.so` 的 `opi_fcitx5_*` C 符号）。键事件路由全部在
Rust 侧（`../src/input_method.rs`，镜像 Android KeyRouter），本文件只做
键事件转发与提交。

## 构建前提

- fcitx5 头与库（≥ 5.1）。**不需要 root、不需要装包**：直接从发行版仓库
  下载 .deb 解包即可（本机实测 deepin 仓库，包版本 `5.1.12-2deepin12`）。
- Rust cdylib 已构建：

```bash
cargo build --release -p fcitx5_opi   # → ../../target/release/libfcitx5_opi.so
```

### 取得头文件与库（可复现）

```bash
cd /tmp && rm -rf fcitx5-hdr && mkdir -p fcitx5-hdr && cd fcitx5-hdr
# 头文件（136 KB）
apt-get download libfcitx5core-dev libfcitx5utils-dev libfcitx5config-dev
# 运行时库（623 KB）—— 链接与 ldd 校验需要
apt-get download libfcitx5core7 libfcitx5config6 libfcitx5utils2
for f in *.deb; do dpkg-deb -x "$f" root/; done
```

解包后：

| 内容 | 路径 |
|------|------|
| 头文件 | `root/usr/include/Fcitx5/{Core,Utils,Config}/` |
| 库 | `root/usr/lib/x86_64-linux-gnu/libFcitx5{Core,Utils,Config}.so` |

`libfcitx5core-dev` 依赖 `libfcitx5core7` + `libfcitx5config-dev` + `libfcitx5utils-dev`，
所以要三个 `-dev` 全下（缺 `Config` 会在 `fcitx/addoninstance.h:13` 报
`fcitx-config/configuration.h` 找不到）。

**不要把这些 .deb / 头文件提交入库**（体积 + 各自许可证；头文件是
LGPL-2.1-or-later，见各文件 SPDX 头）。

## 编译（fcitx5 addon .so）

本机无 fcitx5-dev 时用上面解包出来的路径（下面 `HDR`/`LIB` 即解包目录）：

```bash
HDR=/tmp/fcitx5-hdr/root/usr/include/Fcitx5
LIB=/tmp/fcitx5-hdr/root/usr/lib/x86_64-linux-gnu
INC="-I$HDR/Core -I$HDR/Utils -I$HDR/Config"

# 1) 语法检查
g++ -std=c++17 -fsyntax-only $INC opi_fcitx5.cpp

# 2) 编译成 .o
g++ -std=c++17 -Wall -Wextra -c -fPIC -o /tmp/opi_fcitx5.o $INC opi_fcitx5.cpp

# 3) 链接成插件 .so（首次真正链接通过）
g++ -std=c++17 -shared -fPIC -o libfcitx5_opi_glue.so opi_fcitx5.cpp $INC \
    -L../../target/release -lfcitx5_opi \
    -L$LIB -lFcitx5Core -lFcitx5Utils -lFcitx5Config \
    -Wl,-rpath,'$ORIGIN'
```

装了 `fcitx5-dev` 的机器上，`$INC` 换成 `$(pkg-config --cflags fcitx5)`、
`-L$LIB -lFcitx5...` 换成 `$(pkg-config --libs fcitx5)` 即可。

链接结果（本机实测）：

```
$ LD_LIBRARY_PATH=../../target/release:$LIB ldd -r libfcitx5_opi_glue.so | grep -c 'undefined\|not found'
0
$ nm -D --defined-only libfcitx5_opi_glue.so | grep fcitx_addon_factory
0000000000005d73 T fcitx_addon_factory_instance
```

## 头文件 API 对照（改这份胶水前必读）

fcitx5 5.1.x 的真实 API 与常见误写（本文档上一版全是误写，因为它从未被编译过）：

| 误写 | 真名 | 出处 |
|------|------|------|
| `fcitx/keyevent.h` | `fcitx/event.h` | Core 下无 `keyevent.h`；`KeyEvent` 在 `event.h` |
| `fcitx::InputMethod` | `fcitx::InputMethodEngine` | `inputmethodengine.h:17`，已继承 `AddonInstance` |
| `keyEvent.states()` | `keyEvent.key().states()` | `KeyEventBase` 无 `states()`；`key.h:169` |
| `keyEvent.isRepeat()` | `rawKey().states() & KeyState::Repeat` | 无此方法；`KeyState::Repeat = 1U<<31` |
| `keyEvent.isLongPressed()` | **无对应源**（见下） | 5.1.x 的 `KeyEvent` 无长按概念 |
| `commitString(ic, s)` | `ic->commitString(s)` | `InputContext` 的成员，`inputcontext.h:177` |
| `FCITX_ADDON_FACTORY(X)` 自己定义 X | 必须先声明 `class X : public AddonFactory` | 宏只做 `static X factory;`，`addoninstance.h:193` |
| `updateUserInterface` / `setPreedit` / `setCandidateList` 当作 addon 虚函数 | 都不是虚函数，是引擎**主动调用**的成员 | `InputPanel::setPreedit` / `setCandidateList`（`inputpanel.h:55/68`）、`InputContext::updateUserInterface`（`inputcontext.h:210`）；`InputMethodEngine` 虚表里没有 UI 通路 |
| `updateCandidateList` | **5.1.x 无此符号**（整套头文件 grep 无命中） | 候选表靠 `setCandidateList` + `updateUserInterface` 推送 |

## 输入面板（预编辑 + 候选栏）

`keyEvent` 每次处理完都调 `refreshingPanelFor(ic)`（本文件同名静态函数）：把
Rust 侧的 buffer 当预编辑串（光标置末尾）、当前页候选当候选表推给
`ic->inputPanel()`，再 `ic->updateUserInterface(UserInterfaceComponent::InputPanel)`
通知前端。`reset()` 同样清面板 —— 只清引擎不清面板的话，失焦/切输入法后
预编辑与候选栏会挂在屏幕上。

**客户端这条路上有两条独立通道，缺一条都是半条链路**（按能力位分，互不替代）：

| 通道 | 能力位 | 信号 | 由谁触发 |
|---|---|---|---|
| 整个面板 | `ClientSideInputPanel`(1<<39) | `UpdateClientSideUI` | `updateUserInterface(InputPanel)` |
| 内联预编辑 | `Preedit`(1<<1) | `UpdateFormattedPreedit` | `panel.setClientPreedit(...)` + `ic->updatePreedit()` |
| 提交 | — | `CommitString` | `ic->commitString(...)` |

只推前者（本轮之前就是这样）的后果不只是「少一个信号」：声明了 `Preedit` 的
客户端 composing 状态是空的，**切窗口时已输入的拼音会静默丢失** —— core 对
「有 clientPreedit 的客户端失焦」有默认提交行为（实测：只加 `setClientPreedit`
的变异体在 `FocusOut()` 时收到 `CommitString: 'ni'`，原码 0 条）。

> **已知且已接受的行为，别当 bug 报**：修好之后，「输入拼音 → 切窗口」会把这段
> 拼音**提交进文档**（`CommitString: 'ni'`），而不是丢掉。这是 fcitx5 core 对
> **任何**使用 clientPreedit 的输入法的默认行为，**不是本插件选的** —— 本插件
> 因此与 Linux 上其它输入法行为一致。之所以接受：「看得见、可撤销的提交」换掉
> 「看不见的丢失」，后者更坏。
> 想改这个行为的话先做功课：`CapabilityFlag::ClientUnfocusCommit`(1<<5) 看着像
> 开关，但本次实测客户端**没置这一位也照样提交**，所以置位与否并不就是开关，
> 别照名字猜。

两条通道各由**一个**能力位决定，实测（真守护进程，6 键 `nihao`+space）。

下表数字是 **e2e 日志的行数，不是信号条数** —— `opi_e2e.py` 把每条信号打印两遍（逐键一遍 +
末尾汇总再列一遍），所以 `13` = 逐键 6 + 汇总 7、`12` = 逐键 6 + 汇总 6。
**跨行比较有效（同一把尺），但别把它读成「core 发了 13 条信号」。**

| caps | `UpdateClientSideUI` | `UpdateFormattedPreedit` |
|---|---|---|
| `0x8000000000`（仅 panel） | 13 | 0 |
| `0x8000000010`（panel+FmtPreedit，无 `Preedit`） | 13 | **0** |
| `0x8000000002`（panel+`Preedit`） | 13 | 12 |
| `0x8000000012`（harness 现行） | 13 | 12 |
| `0x0000000012`（无 panel 位） | 0 | 12 |

即：面板只看 bit 39，内联预编辑只看 bit 1（连 `FormattedPreedit`(bit 4) 都不是开关）。
**`Preedit` 位缺了就是没有内联预编辑**，这正是这条链路此前一直半通的原因。

⚠️ 代码里那两处 `if (wantsClientPreedit)` 守卫**不改变上表任何一格**：core 的
`InputContext::updatePreedit()` 自己就查这一位（去掉守卫后 `updatePreeditImpl`
仍被调 0 次，DBus 信号条数逐格不变）。留着只是不让面板里存一个客户端渲染不了的
`clientPreedit`；真正决定客户端收到什么的是 core，别把守卫当成防线。

**候选光标**：`list->setGlobalCursorIndex(0)` 让候选栏有高亮态，否则客户端收到的
候选光标恒为 -1（不画选中态）。⚠️ 必须用 `if (!texts.empty())` 守卫 —— 对**空**候选表
调它会**抛** `std::invalid_argument`（`what()` = `CommonCandidateList: invalid global
index`），而「缓冲非空 + 候选 0 条」是常态（`v` / `zzzz` 这类拼音就没有候选）。实测
三种组合（同一份按键序列 `z z z z`，caps `0x8000000012`）：

| 空候选表上的 `setGlobalCursorIndex(0)` | 结果 |
|---|---|
| **有守卫**（现行） | 4 键正常，该缓冲候选光标 -1，守护进程不受影响 |
| 无守卫 + 有 `catch` | 该键**作废**（异常被吞），面板停在**上一次**的候选表上 |
| 无守卫 + 无 `catch` | 异常越过 FFI 边界 → `abort`，守护进程 pid 消失 |

也就是说这条守卫与下一条「异常不越过 FFI 边界」是**同一件事的两面**：fcitx5 core
不接异常，任何从引擎回调漏出去的异常都是整个输入法挂掉。守卫消掉这个具体的抛点，
`catch` 兜住其余的。

**直通键不做无谓重推**：`action == 0`（ESC/方向键/F5/PageDown 这类）不消费按键、
Rust 侧状态不变，缓冲与上次推出去的一样就直接跳过 —— 否则每次直通按键都要白付
一次跨 FFI + 候选 JSON 序列化/解析 + 客户端重绘。

**异常不越过 FFI 边界**：`keyEvent` / `reset` / `select` 三个入口都套了
`catch (const std::exception &)`。fcitx5 core **不捕获**引擎回调抛出的异常
（实测：裸抛 → `std::terminate` → `abort`，日志里是 `libc.so.6(abort+0xd3)` 回溯，
守护进程 pid 直接消失）。Rust 侧每个导出入口都套了 `catch_unwind`，两侧对齐后
异常降级成「这一次按键无效」。

设计要点：**页码只有 Rust 一份**。`opi_fcitx5_candidates` 返回的已经是切片后的
当前页（`candidate.rs` 的 `PAGE_SIZE`），胶水只把它画出来，翻页由 Rust 路由
（`input_method.rs` 的 PageUp/PageDown → `prev_page`/`next_page`）驱动。若把整份
候选交给 `CommonCandidateList` 自己翻，前端点翻页箭头会与 Rust 页码漂移。
代价是候选表恒为单页（`hasNext()` 恒 false，前端不画翻页箭头）。

## 已知边界

- **长按 ⇧ → Lock 在桌面端不可达**：Rust 侧线格式留了
  `KEY_STATE_LONG_PRESSED`（`../src/input_method.rs:60` = `1<<28`），但 fcitx5
  5.1.x 的普通按键事件没有任何长按信号 —— 唯一的长按概念在
  `VirtualKeyboardEvent::isLongPress()`（`event.h:387`，触屏虚拟键盘专用），
  物理键事件拿不到。胶水因此不合成该位，`handle_shift` 的长按分支
  （→ `shift_long_press`）在 fcitx5 桌面端不会被触发。
  需要 B3 定入口（CapsLock 映射，或按住超时），不要靠猜补一个信号。
- **UI 的 `NextPage`/`PrevPage` 是空操作**（实测：调了没有任何面板更新信号）。
  面板候选表恒单页，前端因此不画翻页箭头，不存在「点了没反应的控件」。
  翻页只走 PageUp/PageDown 键。要让箭头可点，得把整份候选交给 C++ 列表翻页
  并让 `prev()/next()` 回调进 Rust —— 那会把页码变成两份状态，需要先定谁是真源。

## 安装

### 1. addon 元数据（两个 conf，**缺一不可**）

文件已在仓库里（`../data/`，与安装目标同构）：

| 仓库路径 | 装到 |
|---|---|
| `../data/addon/opi_fcitx5.conf` | `$XDG_DATA_HOME/fcitx5/addon/` |
| `../data/inputmethod/opi.conf` | `$XDG_DATA_HOME/fcitx5/inputmethod/` |

```bash
cp -r ../data/* "${XDG_DATA_HOME:-$HOME/.local/share}/fcitx5/"
```

**少了 `inputmethod/opi.conf` 就注册不出输入法**：可用输入法列表是 fcitx5
扫描 `inputmethod/*.conf` 得到的，与 addon 的 `OnDemand` 取什么值无关
（实测 `OnDemand=True/False` 两种都能加载并注册）。

### 2. `.so`（两种落地方式，选一种）

`Library=libfcitx5_opi_glue` 的命名规则是 **`X` → `X.so`，不自动补 `lib`
前缀**，所以文件名必须正好是 `libfcitx5_opi_glue.so`。

**方式 A：装到系统 addon 目录**（需要 root）

```bash
sudo cp libfcitx5_opi_glue.so libfcitx5_opi.so /usr/lib/x86_64-linux-gnu/fcitx5/
```

（发行版路径以 `fcitx5-diagnose` 输出为准；Deepin / Debian 是上面这条。）
`libfcitx5_opi.so` 是 Rust cdylib，胶水靠 `$ORIGIN` rpath 找它 —— **两个
必须放同一目录**。

**方式 B：免 root，用 `FCITX_ADDON_DIRS`**

```bash
export FCITX_ADDON_DIRS=$PWD/addons:/usr/lib/x86_64-linux-gnu/fcitx5
```

> ⚠️ `FCITX_ADDON_DIRS` 是**替换**语义不是追加：它整体取代 addon 搜索路径。
> **必须把系统目录手抄在后面**（上例冒号后那段），否则拼音、搜狗等系统
> 输入法会一起失效。

### 3. 词库

```bash
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/opi"
cp luna.opid "${XDG_DATA_HOME:-$HOME/.local/share}/opi/luna.opid"
```

`loadDictionary()` 经 `fcitx::StandardPath` 探测 `$XDG_DATA_HOME/opi/luna.opid`
（文件名镜像 Android `EngineLoader.FILE_NAME`）。

- **不放词库**：静默回退 Rust 侧**内置 35 词库** —— 能用，但只有几十个词的
  规模，候选质量会明显不如预期。
- **词库存在但损坏**（下载不全、拷贝中断）：同样回退内置词库，但会在
  fcitx5 日志里留一行
  `fcitx5-opi: 词库 <路径> 装载失败，回退内置词库`。
  这是修掉的一个静默失效：原先 `opi_fcitx5_load` 的返回值被丢弃，而 Rust 侧
  `install()` 是 `CandidateState::load(path)?` —— 坏词库在 `?` 处提前返回，
  单例保持 `None`，于是 `with_state` 恒为 `None`，**所有**导出函数退化成空
  操作。表现为插件「已加载、已注册、按键被接受、但一个字都不出」，且任何
  地方都没有错误信息。
  `install(坏路径) -> Err` 是 Rust 侧的**有意**语义（`../src/lib.rs` 的
  `install_singleton_fallback_and_path` 测试就断言了「坏路径 → Err，不回退」），
  所以回退必须由胶水层承接，不要改 Rust 语义。

**装完重启 fcitx5**，然后 `fcitx5-diagnose | grep -i opi` 应能看到 addon 与
输入法条目。

## 验证 harness（**手工跑，不进 CI**）

CI 上既没有 fcitx5 的头/库，也没有 dbus 会话，所以下面这些**都不在 CI 里**，
要人手动跑 —— 但胶水是本仓库唯一「没有单测、又是纯 C++」的部分，跑一次很值：

| 文件 | 验什么 | 要 dbus |
|---|---|---|
| `run-harness.sh` | 一键跑完下面全部 | — |
| `opi_panel_driver.cpp` | 直接构造 `OpiEngine` + 真 `fcitx::InputContext`，逐项断言面板推进（预编辑/clientPreedit/候选/候选光标/翻页/点选/提交/reset/直通键） | 否 |
| `opi_json_check.cpp` + `opi_json_vec.txt` | 狭 JSON 解析器 vs **真 serde_json 输出**，逐字节比对 | 否 |
| `opi_e2e.py` | 真 fcitx5 守护进程里**用户实际收到**的信号（`basic` / `page` / `passthrough` / `caps` 四种模式），每步都是**断言**不是打印 | **要** |

> `opi_json_vec.txt` 是**数据文件不是代码**：它必须逐字节等于 serde_json 的真实
> 输出（第 1 行）与候选串的 hex（第 2 行），加任何注释行都会让解析器读错，
> 所以它**没有**（也不能有）SPDX 头 —— 不是漏了。它的「说明书」写在
> `opi_json_check.cpp` 的头部注释里（含重新生成用的 Rust 片段）。

一条命令：

```bash
FCITX5_HEADERS=/tmp/fcitx5-hdr/root/usr/include/Fcitx5 \
  crates/fcitx5-opi/cpp/run-harness.sh
```

`FCITX5_HEADERS` **必填**，填「**含 `Core/ Utils/ Config/` 三个子目录**」的那一层
（deb 解包是 `…/root/usr/include/Fcitx5`，装了 `fcitx5-dev` 的机器是
`/usr/include/Fcitx5`）。获取办法见本文件开头「取得头文件与库」。
其余环境变量见脚本头部注释（`FCITX5_LIBS` / `FCITX5_SYSTEM_ADDONS` /
`OPI_HARNESS_WORK`，都选填）。

脚本自己会：构建 Rust cdylib → 用 `-Wall -Wextra -Werror` 编译链接胶水（**警告即
失败**，不再靠人看输出）并查 `ldd -r` 与导出符号 → 布置独立的
`XDG_CONFIG_HOME`/`XDG_DATA_HOME`（含**把 opi 放进输入法组**的 profile）与 addons
目录 → 跑完五种 e2e 模式，每个模式**各自一个 `dbus-run-session` + 一个独立守护
进程**（日志分开成 `fcitx5-<n>.log`）→ 最后打印加载记录。**不碰用户真实的 fcitx5
配置，也不会连到用户正在跑的实例上。** 产物与日志留在 `$OPI_HARNESS_WORK`
（缺省 `mktemp -d`）。e2e 断言失败时脚本以非零退出，但退出前仍会打印加载记录。

守护进程是**前台**起的（**不要加 `-d`**）：`-d` 会 daemonize，`$!` 是那个立刻
退出的父进程，`kill $!` 打空（实测返回 1）而真正的守护进程留下来 —— 「起来 →
跑 e2e → 收掉」的后半段全是假的，下一次运行可能落在**旧守护进程**上出结果。
同类的坑还有两个，都会让 e2e **照样出结果**却测的不是你以为的东西：

- 同一 dbus 会话里起第二个 fcitx5，它拿不到 dbus 名、把加载好的 addon 全部卸掉、
  一个字都不服务（日志 `Unable to request dbus name. Is there another fcitx
  already running?`），而 e2e 连上的是**第一个**实例。
- `opi_e2e.py` 里订阅一个**不存在**的信号（曾写作 `UpdatePreedit`，真名是
  `UpdateFormattedPreedit`）不报错、只是永远静默。现在启动时会对着 introspection
  核对信号名，写错直接红。

因此脚本收尾时会确认守护进程**真的没了**（`kill -0` 复查一次），加载记录里也应
能看到配对的 `Unloading addon opi_fcitx5` —— 只有被真 pid 收到 SIGTERM 才会打这行。

`opi_panel_driver.cpp` 第 3 节有一步会打 `[SKIP]` 而不是 `[PASS]`：缓冲 `nihao`
在词库下常常不足 8 条候选，翻页无从观察 —— 这是词库规模决定的，不是失败。

## 状态

**编译/链接**：在 fcitx5 5.1.12（deepin 仓库解包头 + 库）下通过
（`-fsyntax-only`、`g++ -c`、`g++ -shared` 三级全过，`ldd -r` 零未定义符号）。
本文件此前从未被任何编译器看过，含 7 处 API 误写（见上表），已按真实头文件
逐条修正；`OpiEngineFactory` 此前全仓库不存在，已补。

**运行（真实 fcitx5 5.1.12）**：本目录 `data/` 的两个 conf + 上面方式 B 的
`FCITX_ADDON_DIRS`，在私有 dbus 会话里起 `fcitx5 --ui=testui` 实测：

- `Loaded addon opi_fcitx5`（addon 加载成功）
- `AvailableInputMethods()` 里出现 `('opi', 'OPI 拼音', ..., 'zh_CN', ...)`
  （输入法条目注册成功）
- 把词库换成 6 字节垃圾时，日志出现上面那行装载失败告警

**输入面板（端到端已跑通）**：在私有 dbus 会话里起真 fcitx5，经 xdg-desktop-portal
输入法接口（`org.fcitx.Fcitx.InputMethod1` 的 `CreateInputContext`）建输入上下文，
声明 `Preedit | FormattedPreedit | ClientSideInputPanel`（`1<<1 | 1<<4 | 1<<39`），
`FocusIn` → 送按键。收到的信号：

```
Loaded addon opi_fcitx5
UpdateFormattedPreedit: preedit="n"     预编辑光标=1
UpdateClientSideUI: preedit="n" 预编辑光标=1 候选光标=0 候选=8 ['你','那','能','年','呐','呢','吶','您']
UpdateFormattedPreedit: preedit="ni"    预编辑光标=2
UpdateClientSideUI: preedit="ni" 预编辑光标=2 候选光标=0 候选=8 ['你','泥','拟','擬','呢','腻','祢','铌']
UpdateClientSideUI: preedit="nihao" 预编辑光标=5 候选光标=0 候选=6 ['你','好','号','號','泥','拟']
键 PageDown -> 候选光标=0 候选=8 ['内','內','哪','农','農','难','難','女']
键 PageUp   -> 候选光标=0 候选=8 ['你','那','能','年','呐','呢','吶','您']
SPACE -> CommitString: 你
      -> UpdateFormattedPreedit: preedit=""   （紧接着一条全空的面板更新）
```

即：预编辑串、**内联预编辑**、候选列表、**候选光标**、翻页、提交后清屏**都在真
守护进程里被真正的 fcitx5 推给了客户端**，不只是构造 `OpiEngine` 的自测。

上面这段是**浓缩**（真跑出来每条信号还带 `CurrentIM` 与逐键分节）。要复跑出
原始输出，跑 `crates/fcitx5-opi/cpp/run-harness.sh`（见上「验证 harness」），
它会原样打印 `basic` / `page` / `passthrough` / `caps 0x12` /
`caps 0x8000000012` 五次会话的全部
信号。

> ⚠️ 之前「按键进不了引擎（`handled=0`）」的**根因不是焦点**：`FocusIn()`
> 只是把输入法设成当前**输入法组里的当前项**（用户 profile 里是
> keyboard-us），而 **`SetCurrentIM("opi")` 在 opi 不属于该组时静默无效** ——
> 不报错、不切换、CurrentIM 信号也不发，看上去就像「插件坏了」。
> profile 的组里加上 opi（用独立的 `XDG_CONFIG_HOME`，**没有碰用户真实
> 配置**）后，`CurrentIM` 变成 `['OPI 拼音','opi','zh_CN']`，按键立刻进引擎。
>
> 另一条同类陷阱：**面板去哪儿由能力位决定，而且是两条独立通道**（整个面板看
> `ClientSideInputPanel`(1<<39)，内联预编辑看 `Preedit`(1<<1)）。实测对照（同一串
> 按键，只改能力位）：
>
> | capability | 客户端收到 |
> |---|---|
> | `0x12`（Preedit\|FormattedPreedit） | 逐键 `UpdateFormattedPreedit`（内联预编辑）；**没有** `UpdateClientSideUI` |
> | `0x8000000012`（多一位 ClientSideInputPanel） | 上面那条**加上**逐键 `UpdateClientSideUI`：预编辑 + 光标 + 候选 + 候选光标 |
>
> `0x1FF` 里**没有** `ClientSideInputPanel` 那一位 —— 早先「按了键什么都没显示」
> 很可能是把面板送去了 UI 侧而只盯着客户端看（或反过来）。另注：`--ui=testui`
> 不打印面板内容，所以**UI addon 那条投递路径没有可观测记录**，可观测的是客户端
> 那两条。
>
> 这两条通道曾经只推了第一条，而当时的 `opi_e2e.py` **订阅的信号名是错的**
> （写 `UpdatePreedit`，真名 `UpdateFormattedPreedit`）—— 订阅不存在的信号不报错、
> 只是永远静默，于是「内联预编辑没通」既没被 harness 抓到，还被读成了「预编辑已
> 验证」。现在信号名启动时对着 introspection 核对，且每步都是断言。

键事件路由与候选提交另有**直接构造 `OpiEngine`** 的驱动验证（同一份胶水源码
+ 同一个 Rust `libfcitx5_opi.so`，走真实 `loadDictionary()` 与
`fcitx::InputMethodEngine` 虚接口）：

| `$XDG_DATA_HOME/opi/luna.opid` | `h`/`a`/`o`/`SPACE` 的 action |
|---|---|
| 不存在 | 1 / 1 / 1 / **2（text=`好`）** 内置回退 |
| 真词库 1.7MB | 1 / 1 / 1 / **2（text=`好`）** |
| 6 字节垃圾（修复前） | **0 / 0 / 0 / 0**，无提交 |
| 6 字节垃圾（修复后） | 1 / 1 / 1 / **2（text=`好`）** + 告警 |

action 的取值含义见本文件开头（0=直通 1=已处理 2=提交）；`action=2` 且
`text='好'` 在胶水里对应 `ic->commitString("好")`。

