// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 运行期面包屑（2026-10-23）。用户报「点英文键 / 面板返回后跳到系统键盘」，
// 而源码里这两条路都**不调** `advanceToNextInputMode()`；下一层嫌疑是扩展被
// 系统按内存击杀或崩溃（表现就是系统键盘顶上来）。真机上用 Console.app 按
// subsystem 过滤，看最后一条停在哪一步 —— 没有日志就只能继续猜。
//
// category 分开是为了过滤：input-mode（切模式/切英文）、panel（开关面板）、
// memory（内存警告）。全部走 `os.Logger`（iOS 14+，部署目标是 15.0）。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；日志点本身没有行为。

import os

enum OpiLog {
    private static let subsystem = "xyz.erik.opi.keyboard"

    static let input = Logger(subsystem: subsystem, category: "input-mode")
    static let panel = Logger(subsystem: subsystem, category: "panel")
    static let memory = Logger(subsystem: subsystem, category: "memory")
}
