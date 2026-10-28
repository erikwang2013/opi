// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 用户词落盘（2026-10-28）。此前引擎的学习只活在内存里，扩展进程一退就全忘。
//
// 路径：<扩展容器>/Library/Application Support/opi/opi_user_words.json —— 文件名与
// 格式跟 macOS `OpiSettings.swift`、fcitx5、Android `UserWordStore` 同一套（JSON 由
// 引擎导出/导入，本层**不解析**它）。扩展没有 App Group：这份文件只在键盘扩展自己的
// 容器里，宿主 App 看不到。
//
// 不变式：读/写/导入/导出的任何失败都只 NSLog、不崩、不删原文件 —— 键盘扩展崩掉
// 就是跳回系统键盘，比丢一次学习严重得多；坏文件保留到下一次成功落盘才原子替换。
//
// ⚠️ 本机无 UIKit/Apple SDK，只在 Apple CI 真编译；写盘行为未上机验证。

import Foundation

enum UserWordsStore {
    /// 尾随防抖窗口，对齐 Android `UserWordStore.SAVE_DEBOUNCE_MS`（1.5s）。
    private static let saveDelay: TimeInterval = 1.5

    /// 写盘串行队列。多个控制器实例（切换宿主 App 时）共享同一个文件与同一个
    /// `pendingSave`，串行才保证「最后一次导出最后落盘」。
    private static let queue = DispatchQueue(label: "xyz.erik.opi.ios.user-words")
    private static var pendingSave: DispatchWorkItem?

    /// `<Application Support>/opi/opi_user_words.json`。
    private static var fileURL: URL {
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory())
        return base.appendingPathComponent("opi", isDirectory: true)
            .appendingPathComponent("opi_user_words.json")
    }

    /// 启动恢复：把上次导出的 JSON 导回引擎（词库装载后调用一次）。
    /// 文件缺失 = 冷启动；0 字节 / 非 UTF-8 / 引擎拒收 → 日志 + 保留原文件。
    static func load(into engine: OpiEngine) {
        let url = fileURL
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        do {
            let data = try Data(contentsOf: url)
            guard !data.isEmpty else {
                NSLog("[OPI] 用户词文件是 0 字节（掉电残留？），保留原文件：%@", url.path)
                return
            }
            guard let json = String(data: data, encoding: .utf8) else {
                NSLog("[OPI] 用户词文件不是 UTF-8，保留原文件：%@", url.path)
                return
            }
            let count = engine.importUserWords(json: json)
            guard count >= 0 else {
                // -1 也包含「词库没装上、引擎单例为空」—— 文件保留，下次启动再试。
                NSLog("[OPI] 用户词导入失败（引擎未就绪或文件损坏/版本不符），保留原文件：%@",
                      url.path)
                return
            }
            NSLog("[OPI] 已载入 %d 条用户词：%@", count, url.path)
        } catch {
            NSLog("[OPI] 用户词文件读不出（%@），保留原文件", error.localizedDescription)
        }
    }

    /// 一次上屏后调用（主线程）。导出留在调用线程（引擎调用不跨线程），只把文件
    /// 系统动作放进队列；连续上屏按尾随防抖合并成一次写盘。
    static func markDirty(_ engine: OpiEngine) {
        // 引擎未装载时导出是空串；写成 0 字节文件会被下次启动判成损坏。
        let json = engine.exportUserWords()
        guard !json.isEmpty else { return }
        queue.async {
            pendingSave?.cancel()
            let work = DispatchWorkItem { write(json) }
            pendingSave = work
            queue.asyncAfter(deadline: .now() + saveDelay, execute: work)
        }
    }

    /// 立即落盘（键盘收起/切 App 时调用，主线程）：防抖回调可能等不到进程被杀。
    /// 同步等写盘完成 —— 文件只有几 KB，换「收起后这次学习一定在」。
    static func flush(_ engine: OpiEngine) {
        let json = engine.exportUserWords()
        guard !json.isEmpty else { return }
        queue.sync {
            pendingSave?.cancel()
            pendingSave = nil
            write(json)
        }
    }

    /// 清掉落盘副本（设置面板「清除用户词库」；引擎内存由控制器清）。
    /// **先取消在途写盘再删文件** —— 否则清完可能被 1.5 秒后的旧快照写回来。
    static func clear() {
        queue.sync {
            pendingSave?.cancel()
            pendingSave = nil
            let url = fileURL
            guard FileManager.default.fileExists(atPath: url.path) else { return }
            do {
                try FileManager.default.removeItem(at: url)
            } catch {
                NSLog("[OPI] 用户词文件删除失败（%@）：%@", url.path, error.localizedDescription)
            }
        }
    }

    /// 仅在 queue 上调用。原子写：Foundation 先写临时文件再 rename，
    /// 写一半被杀时旧文件仍完好。
    private static func write(_ json: String) {
        let url = fileURL
        do {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data(json.utf8).write(to: url, options: .atomic)
        } catch {
            NSLog("[OPI] 用户词写入失败（%@）：%@", url.path, error.localizedDescription)
        }
    }
}
