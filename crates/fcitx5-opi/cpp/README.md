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

## 已知边界

- **长按 ⇧ → Lock 在桌面端不可达**：Rust 侧线格式留了
  `KEY_STATE_LONG_PRESSED`（`../src/input_method.rs:60` = `1<<28`），但 fcitx5
  5.1.x 的普通按键事件没有任何长按信号 —— 唯一的长按概念在
  `VirtualKeyboardEvent::isLongPress()`（`event.h:387`，触屏虚拟键盘专用），
  物理键事件拿不到。胶水因此不合成该位，`handle_shift` 的长按分支
  （→ `shift_long_press`）在 fcitx5 桌面端不会被触发。
  需要 B3 定入口（CapsLock 映射，或按住超时），不要靠猜补一个信号。

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

**未经端到端验证的部分**：在 fcitx5 守护进程内**按下真实按键并看到
`CommitString`** 这一段没跑通 —— 经 dbus `ProcessKeyEvent` 送键时输入法
上下文拿不到焦点，按键未进入引擎（`handled=0`，与词库好坏无关；且
`FocusIn()` 会把当前输入法切回 `keyboard-us`）。故键事件路由与候选提交是在
**直接构造 `OpiEngine`** 的驱动里验证的（同一份胶水源码 + 同一个 Rust
`libfcitx5_opi.so`，走真实 `loadDictionary()` 与 `fcitx::InputMethodEngine`
虚接口）：

| `$XDG_DATA_HOME/opi/luna.opid` | `h`/`a`/`o`/`SPACE` 的 action |
|---|---|
| 不存在 | 1 / 1 / 1 / **2（text=`好`）** 内置回退 |
| 真词库 1.7MB | 1 / 1 / 1 / **2（text=`好`）** |
| 6 字节垃圾（修复前） | **0 / 0 / 0 / 0**，无提交 |
| 6 字节垃圾（修复后） | 1 / 1 / 1 / **2（text=`好`）** + 告警 |

action 的取值含义见本文件开头（0=直通 1=已处理 2=提交）；`action=2` 且
`text='好'` 在胶水里对应 `ic->commitString("好")`。

