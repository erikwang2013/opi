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
//
// ✅ 首编（CI run 37933881578，2026-10-09）确认了一条 API 事实：这个 init 是
// **可失败的**（`IMKServer?`），草案原样写成 `let server = ...` 会被编译器拦下：
//   error: value of optional type 'IMKServer?' must be unwrapped to a value of type 'IMKServer'
// 失败语义（文档 + 按最坏打算）：连接名已被占用时返回 nil（例如上一个实例还在跑）。
// 没有 server 的输入法什么也做不了，所以这里与上面缺 CFBundleIdentifier 同一形态：
// 打日志 + 退出，不静默降级、也不用 `!` 直接崩（崩掉的进程没有任何可读原因）。
// 仍未验证：是否需要先起 NSApplication（见文件尾）。
guard let server = IMKServer(name: connectionName, bundleIdentifier: bundleID) else {
    NSLog("opi: IMKServer 建不起来 —— 连接名 \(connectionName) 可能已被占用（另一个实例还在跑？）")
    exit(1)
}

// 候选窗依赖 server（IMKCandidates 的 init 要它）。
OpiCandidateWindow.shared.configure(server: server)

// 词库启动时装一次（对照 fcitx5 轨在引擎构造里 loadDictionary()）。
// 失败会回退内置词库并打日志，不会让进程起不来 —— 输入法「装上了但不出字」
// 比「起不来」难查得多，故这里只记日志，不退出。
OpiEngine.shared.loadDictionary()
// 可选繁体库（同目录 trad.opid；缺文件就保持简体回退）。
OpiEngine.shared.loadOptionalTradDictionary()
// 可选笔画/五笔码表（Resources/dicts/ 或用户目录；缺文件只记日志）。
// 装载**不切换方案**：当前方案仍由 `applyPersisted()` 决定。
OpiEngine.shared.loadOptionalStrokeDictionary()
OpiEngine.shared.loadOptionalWubiDictionary()
// 用户词（学习结果）从 dataDirectory/opi_user_words.json 恢复到已装载的引擎。
// 缺失 = 冷启动；坏文件只记日志并保留（见 OpiSettings.swift 的持久化段）。
OpiEngine.shared.loadPersistedUserWords()
// 用户设置的套用必须**在词库之后**：opi_load 之后引擎才是可写状态。
OpiSettingsStore.applyPersisted()
// 跨设备剪贴板（默认关）：按落盘开关启动；只起网络/剪贴板轮询，不记录内容。
OpiClipService.shared.applyPersisted()
// 导入词库（dicts/*.tsv）按停用名单重放；坏文件只记日志，不挡启动。
OpiEngine.shared.loadImportedDicts(disabled: OpiSettingsStore.disabledDicts())
// 自定义短语：宿主文件是唯一真源，引擎只持内存，启动时中英两套整表重推（坏文件按空表）。
OpiCustomPhraseStore.shared.loadAndPush(.chinese)
OpiCustomPhraseStore.shared.loadAndPush(.english)

// 后台常驻：本进程没有窗口，靠 run loop 收 IMK 事件。
// ⚠️ 未验证：LSBackgroundOnly 下用 NSApplication.shared.run() 是否正确
// （备选：RunLoop.main.run()，或 RunLoop.current.run() 配一个空 port）。
NSApplication.shared.run()
