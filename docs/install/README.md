<!-- SPDX-FileCopyrightText: 2026 erik.xyz -->
<!-- SPDX-License-Identifier: MIT -->

# 各端安装教程 —— 索引

> **文档版本（2026-11-01，v1.29.0）**：动态皮肤已铺到各端面板/选择器/设置页；
> 候选词会按词名反查补符号/颜文字；跨设备剪贴板已接线（六端默认关闭，能力边界
> 见下方「跨设备剪贴板」）。语音输入仍在开发，未发布。

> ## ⚠️ 装完之后必做
>
> **重启 fcitx5（或注销重登），然后在输入法设置里把 OPI 加进「输入法组」。**
>
> **不做这一步，输入法列表里看得到 OPI、但打字什么都不会发生。** 新装的输入法**不会
> 自动启用** —— 装上不等于选上，这是本项目里最常被当成「安装无效」的一步。
>
> 逐条命令、两个打开入口、以及「列表里看不到它」怎么查，只写在
> [`linux.md`](linux.md) 的 §4 里；**本页不复述那一节**（两份会漂）。
> Android 的对应步骤在 [`android.md`](android.md) 的 §4.1（在系统设置里启用输入法）。

## 各端教程

| 平台 | 适用情形 | 教程 |
|---|---|---|
| **Linux**（fcitx5 插件） | 想在自己桌面上真用起来。**目前唯一有 CI 在真 fcitx5 上验过「装完会不会被加载」**的一端，并产出 `.deb` / `.rpm` | [`linux.md`](linux.md) |
| **Android**（原生 IME） | 有 Android SDK + NDK 和一台设备 / 模拟器。正文多数条目是【实测】；早前「`assembleDebug` 本机失败（缺 Dart SDK）」已订正（2026-10-11）——debug/release 均可构建，**分发口径是 `assembleRelease`**（debug 的 Rust 未优化，见该篇 §2.3） | [`android.md`](android.md) |
| **Windows**（TSF 插件） | 想在 Windows 上趟这一端。CI 已构建用户入口 **EXE 安装器** `opi-<版本>-windows-x64-setup.exe`（默认装到 `C:\Program Files\opi`，TSF 注册失败不回滚），并用容器做静默安装/卸载冒烟；后处理 MSI 仅为内部证据。**安装/打字/候选窗/中英文/状态栏已在 Windows 10 Home 真机验收（2026-10-21）；Windows 10 Pro / Enterprise 与 Win11 待真机验收；交互式向导与语言栏图标仍未验收；仅 Win10 1607+/11 x64** | [`windows.md`](windows.md) |
| **iOS**（键盘扩展） | 有 macOS + Xcode、想接着往下做。CI 已真编译 Swift，并产出**宿主 App + unsigned IPA**；Release / 站点也附 `opi-<版本>-ios-unsigned.ipa`，签名与真机运行未验证 | [`ios.md`](ios.md) |
| **macOS**（InputMethodKit） | 需要 macOS + Xcode。CI 已真编译 Swift 并产出 **unsigned pkg**（`opi-<版本>-macos-universal.pkg`，`.app` 只在构建期存在）；Release / 站点同款，签名、公证与真机运行未验证 | [`macos.md`](macos.md) |
| **鸿蒙**（ArkTS + N-API） | 想接着做鸿蒙端。工程脚手架已补齐，可跑 `hvigorw assembleHap` 并产出 **unsigned HAP**（Release / 站点附 `opi-<版本>-harmonyos-unsigned.hap`）；签名与真机未验证 | [`harmony.md`](harmony.md) |

> **本页只做导引。** 每篇开头都有一张「这份文档的性质」表，写清哪一层是实测、哪一层
> 从未被编译器看过 —— 「适用情形」这一列是导引式概括，**状态一律以各篇自己的表述为准**。

## 跨设备剪贴板（2026-11-01，默认关闭）

六端共用 `crates/opi-clip` 内核：UDP 发现、Noise_XX + 6 位 SAS 配对、Noise_KK 会话、
TCP 分片/ACK/去重/限速；**只同步 UTF-8 纯文本，单条 ≤256 KiB**，内容不经云端、不写入
历史、不落日志；身份/信任文件只存设备公钥与配置。配对必须两端显示并确认同一组
**6 位数字**；可一键停止共享、逐台撤销设备。

**各端「能不能自动监听」不是一回事，别按「六端无缝」理解**：

| 平台 | 自动监听 | 降级 / 边界 |
|---|---|---|
| **Android** | 是（默认输入法进程内，另由设置页拉起 `connectedDevice` 前台服务） | Android 10+ 只有前台/默认输入法能读本机剪贴板；Android 13+ 拒绝通知权限只让通知不可见，共享仍运行 |
| **Windows 10 1607+ / 11 x64** | 是（`CF_UNICODETEXT` 轮询） | 首次监听弹 Windows 防火墙；安装器**不加**规则；两台真机同步未验证 |
| **Linux** | X11 是（AWT 轮询）；**Wayland 否** | Wayland（尤其 GNOME）没有全局监听，只能在设置页「发送当前剪贴板」；两机同步未验证 |
| **macOS** | 是（`NSPasteboard` 轮询，约 0.5s） | macOS 15+ 首次监听请求「本地网络」；未上机 |
| **iOS** | **否**（仅键盘可见时） | 键盘收起即停、无后台；收到内容要点「插入」，发送要点工具条；依赖「完全访问」与本地网络授权；未上机 |
| **鸿蒙** | **否**（仅输入会话可见时） | 读剪贴板受系统限制，设置页「粘贴」按钮手动同步一次；基础体验模式会限制后台网络/剪贴板；未上机 |

> **验证口径**：以上只有协议/适配器单测与各端 JVM 侧测试；**没有任何一端做过两台
> 真机的配对与同步验收**。每篇的「已知限制 / 未验证清单」为准。

## 可选：导入第三方词库

CustomPinyinDictionary（CC BY-SA 4.0）与 rime-wanxiang（CC BY 4.0）的离线转换脚本、
各端加载路径、条目/大小上限与许可义务，见
[`../external-dictionaries.md`](../external-dictionaries.md)。**这不是随包数据**：仓库
只含几十行级测试夹具，完整词库需用户自己生成并放进词库目录。

> **证据档位**（各篇通用）：**【实测】** 在本机或本机容器里真跑过 · **【推断】** 从源码
> 读出来的结论 · **【未验证】** 本机够不到，没有任何人跑过这一步。本页提到的 GUI 操作
> 路径**均未在本机 GUI 验证**（本机没有图形会话）。
