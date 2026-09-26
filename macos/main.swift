// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器、链接器或语法检查。
// 见 macos/README.md「最不确定的 API」#5（启动顺序 / run loop）。
//
// main.swift —— IMKServer 启动入口（无 nib、无 storyboard）。
// Info.plist 里 LSBackgroundOnly=true，本进程没有界面、没有 Dock 图标。
//
// 另一种做法是 nib 驱动（NSPrincipalClass + MainMenu.nib，由系统装配
// IMKServer）：本端不用 nib —— 启动路径少一个无法在 Linux 上核对的
// 二进制资源。代价是 IMKServer 必须在这里手工建，且连接名必须与
// Info.plist 的 InputMethodConnectionName 逐字一致，否则系统连不上。

import Cocoa
import InputMethodKit

/// CFBundleIdentifier 必须存在：IMKServer 靠它把连接归到本输入法。
/// 直接从命令行跑可执行文件（而不是 .app）时会拿不到，故显式判一次，
/// 否则症状是「进程起来了但系统里没有这个输入法」，很难查。
guard let bundleID = Bundle.main.bundleIdentifier else {
    NSLog("opi: 没有 CFBundleIdentifier —— 请从 .app 启动，不要直接跑可执行文件")
    exit(1)
}

let connectionName = (Bundle.main.infoDictionary?["InputMethodConnectionName"] as? String)
    ?? "OpiInputMethod_Connection"

// 启动即建 server：系统随后为每个客户端连接创建 OpiInputController 实例。
// ⚠️ 未验证：init 签名（name:bundleIdentifier:）与「是否需要先起 NSApplication」。
let server = IMKServer(name: connectionName, bundleIdentifier: bundleID)

// 候选窗依赖 server（IMKCandidates 的 init 要它）。
OpiCandidateWindow.shared.configure(server: server)

// 词库启动时装一次（对照 fcitx5 轨在引擎构造里 loadDictionary()）。
// 失败会回退内置词库并打日志，不会让进程起不来 —— 输入法「装上了但不出字」
// 比「起不来」难查得多，故这里只记日志，不退出。
OpiEngine.shared.loadDictionary()

// 后台常驻：本进程没有窗口，靠 run loop 收 IMK 事件。
// ⚠️ 未验证：LSBackgroundOnly 下用 NSApplication.shared.run() 是否正确
// （备选：RunLoop.main.run()，或 RunLoop.current.run() 配一个空 port）。
NSApplication.shared.run()
