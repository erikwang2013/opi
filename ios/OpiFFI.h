// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过 clang / 未经过 Xcode / 未经过任何 Swift 编译器。
//
// ===== 这是一份**转发头**，不是声明 =====
//
// C ABI 的**唯一一份**声明在 ../macos/OpiFFI.h（逐条对照
// crates/opi-ffi/src/cabi.rs 的 28 个 C 导出抄写）。
//
// ⚠️ 别把「28」当成长期常量：cabi.rs 还在长（历史 19 → 20 → 22 → 28，
// 最近一次扩容是 861b7a7；**从没有过 25**），
// 而这份转发头**没有**自己的版本号 —— 声明头落后于 Rust 侧时，
// Swift 调新出口就是「编译不过」，不是静默错。以 cabi.rs 为准。
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
