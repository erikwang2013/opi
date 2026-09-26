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

- **能编译能链接，不能运行**：运行需要真实 fcitx5 实例（`fcitx5` 进程加载本
  插件）。本机没有可用的 fcitx5 会话，故只有编译/链接级验证，**无运行时验证**。
- **长按 ⇧ → Lock 在桌面端不可达**：Rust 侧线格式留了
  `KEY_STATE_LONG_PRESSED`（`../src/input_method.rs:60` = `1<<28`），但 fcitx5
  5.1.x 的普通按键事件没有任何长按信号 —— 唯一的长按概念在
  `VirtualKeyboardEvent::isLongPress()`（`event.h:387`，触屏虚拟键盘专用），
  物理键事件拿不到。胶水因此不合成该位，`handle_shift` 的长按分支
  （→ `shift_long_press`）在 fcitx5 桌面端不会被触发。
  需要 B3 定入口（CapsLock 映射，或按住超时），不要靠猜补一个信号。
- 插件元数据（`opi_fcitx5.conf` 等）与词库分发仍是 B3 未接线部分。

## 安装

- `libfcitx5_opi_glue.so` 与 addon 元数据（`opi_fcitx5.conf.in` 等，B3
  接线）放入 fcitx5 addon 目录，例如
  `/usr/lib/fcitx5/`（发行版常见路径，以 `fcitx5-diagnose` 输出为准）。
- 词库路径：B3 的 `opi_fcitx5_init_dict` 把插件分发的 `luna.opid` 经 size
  校验拷贝到 XDG 数据目录（`$XDG_DATA_HOME/opi/luna.opid`，文件名镜像
  Android EngineLoader.FILE_NAME）；初始化时经 `fcitx::StandardPath` 探测
  同一路径；未找到则 Rust 侧使用内置回退词库（打包接线在验收阶段完成）。

## 状态

已在 fcitx5 5.1.12（deepin 仓库解包头 + 库）下**编译通过并链接成 .so**
（`-fsyntax-only`、`g++ -c`、`g++ -shared` 三级全过，`ldd -r` 零未定义符号）。
此前本文件从未被任何编译器看过，因此含 7 处 API 误写（见上表），已按真实
头文件逐条修正；`OpiEngineFactory` 此前全仓库不存在，已补。

**运行**未验证：需要真实 fcitx5 会话加载本插件，本机不具备。验收阶段需在
装有 fcitx5 的桌面机上做运行时验证（键事件路由、候选提交、词库分发）。
