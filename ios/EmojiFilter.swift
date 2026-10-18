// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **未编译**（2026-10-18 从 `EmojiPanel.swift` 拆出；本机没有 swiftc）
//
// 引擎表情表 → 「本机能画的列表 + 与之对齐的分类条」。
//
// 为什么单独一个文件：`EmojiPanel.swift` 顶到 500 行门禁，而这段是**纯函数**，
// 与面板状态无关。分类条下标必须跟着过滤后的列表重算 —— 两步必须一起做，
// 所以合成一个函数；分成两个就给了「只调其中一个」的机会，那是静默错位。

import Foundation

enum EmojiFilter {

    /// 把引擎的**原始**表情表与原始分类条，变成「本机能画的列表 + 与之对齐的分类条」。
    ///
    /// 逐条对齐 Android `SymbolCatalog.emojiGroups`（前缀存活表）：
    /// `aliveUpTo[i]` = 原始表 `[0, i)` 里有多少条通过了 `canRender`；组的 `start`/`count`
    /// **一起**重算（`count` 是分类条读屏文案，不跟着减就会把摘掉的条数也报进去）。
    ///
    /// ⚠️ 表拿不到（未装载 / 没取到）⇒ **原样返回，不重算**：没有列表就没有「过滤后下标」
    /// 可言，硬算会把所有组的 `start` 压成 0（那比不显示分类更坏）。同 Android。
    static func filterAndRegroup(
        _ rawList: [String], groups raw: [OpiEmojiGroup]
    ) -> (list: [String], groups: [OpiEmojiGroup]) {
        // 字形过滤用 SymbolPanel 那一份判据（**只判单码位**，多码位一律放行）——
        // 探针字体也是它的那一颗，别在别处另造（见 SymbolPanel.probeFont 的注释）。
        //
        // ⚠️ 未编译：每条都调一次 `CTFontCreateWithName`（在 canRender 里）。687 条
        // 一次性跑，Mac 上量一下开面板的耗时；若肉眼可见卡，把 CTFont 提到循环外
        // （要改 canRender 的签名，**别**在这里抄一份判定逻辑）。
        var aliveUpTo = [Int](repeating: 0, count: rawList.count + 1)
        var alive = 0
        var list: [String] = []
        list.reserveCapacity(rawList.count)
        for i in rawList.indices {
            aliveUpTo[i] = alive
            if SymbolPanel.canRender(rawList[i], font: SymbolPanel.probeFont) {
                alive += 1
                list.append(rawList[i])
            }
        }
        aliveUpTo[rawList.count] = alive
        guard !rawList.isEmpty else { return (list, raw) }
        let groups = raw.map { g -> OpiEmojiGroup in
            let from = min(max(g.start, 0), rawList.count)
            let to = min(max(g.start + g.count, 0), rawList.count)
            return OpiEmojiGroup(
                name: g.name, start: aliveUpTo[from], count: aliveUpTo[to] - aliveUpTo[from]
            )
        }
        return (list, groups)
    }
}
