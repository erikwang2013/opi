// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过 clang / 未经过 Xcode / 未经过任何 Swift 编译器。
//
// ===== 这是一份**转发头**，不是声明 =====
//
// C ABI 的**唯一一份**声明在 ../macos/OpiFFI.h（逐条对照
// crates/opi-ffi/src/cabi.rs 的全部 C 导出抄写；**数量以
// crates/opi-ffi/tests/c_abi_contract.rs 的那道门禁为准，这里不写数字**）。
//
// ⚠️ **这条警告已被兑现一次**：cabi.rs 走过 19 → 20 → 22 → 28 → 31
// （**从没有过 25**；861b7a7 是其中一次扩容）。2026-09-27 那次扩容让
// ../macos/OpiFFI.h 一度落后 3 个 —— 而正如这里预言的，Swift 调新出口是
// 「**编译不过**」而不是静默错（同日已补齐，见 macos/README.md 的订正）。
// ⇒ 所以这里**不写数量**：写死的数字会随下次扩容再假一遍。
// 这份转发头**没有**自己的版本号 —— 以 cabi.rs 与那道门禁为准。
// 这里**故意不**重抄一遍：本项目已经被「同一语义抄多份」坑过
// （键路由表抄了三份 → 见 crates/engine-core/src/router.rs 模块头注释），
// 再抄一份 C 声明就是第四份。
//
// 用法（Xcode，键盘扩展 target）：
//   Build Settings → SWIFT_OBJC_BRIDGING_HEADER = ios/OpiFFI.h
// Swift 侧即可直接看到 opi_load / opi_key_event / OpiString / OpiKeyEventResult。
//
// ⚠️ 待接手人在 Mac 上决定的事（本机无法判断）：
//   ../macos/OpiFFI.h 这个名字带平台前缀，但内容是**平台中立**的。
//   更合适的家是 `include/opi_ffi.h` 之类的中立位置，macOS / iOS 各自转发。
//   移动它属于仓库结构调整，超出本次「只新建 ios/」的范围，故保留现状。
//   若移动，本文件的相对路径要同步改。

#pragma once

// 相对路径按**本文件所在目录**解析 —— 只在「整个仓库一起 checkout」时成立。
// 单独拷 ios/ 出去会断（这是有意的：断了比悄悄用一份陈旧副本好）。
#include "../macos/OpiFFI.h"
