// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **未编译**（2026-10-18 从 `EmojiPanel.swift` 拆出；本机没有 swiftc）
//
// 引擎表情表 → 「本机能画的列表 + 与之对齐的分类条」。
//
// 为什么单独一个文件：`EmojiPanel.swift` 顶到 500 行门禁，而过滤本身是**纯函数**，
// 与面板状态无关。分类条下标必须跟着过滤后的列表重算 —— 两步必须一起做，
// 所以合成一个函数；分成两个就给了「只调其中一个」的机会，那是静默错位。
//
// 2026-10-30：加了进程级缓存（`browse`）。面板关闭会 `releaseContent()` 清空自身，
// 缓存不能挂在面板状态上；表情两出口是编译期静态表，非空结果永远有效。

import Foundation

enum EmojiFilter {

    /// 已过滤的浏览数据（`(列表, 对齐后的分类)`），**跨面板开关存活**。
    ///
    /// 一次过滤要逐条跑 689 次 CoreText 字形探测（`SymbolPanel.canRender`），
    /// 没有理由每次开面板重做。表情两出口是编译期静态表、未装载也返回全量
    /// （见 `OpiEngine.emojiSymbols()` 的注释），所以非空结果无需失效。
    ///
    /// ⚠️ 空的**原始**表绝不进缓存：空 = 没拿到，不是真的没有（Android
    /// `SymbolCatalog` 踩过的坑）。只缓存 `filterAndRegroup` 的产物。
    private static var browseCache: (list: [String], groups: [OpiEmojiGroup])?

    /// 取浏览数据：命中缓存直接复用（不现取原始表、不重新探测字形）；未命中才调
    /// `fetch` 取原始表并过滤。
    ///
    /// 为什么收闭包：`fetch` 要打引擎出口，而本文件不该认识 `OpiEngine`；
    /// 「空表不缓存」的判据也收在这里，别让调用方各自判一遍。
    /// UI 主线程调用即可（同 `OpiEngine` 的非线程安全假设）。
    static func browse(
        _ fetch: () -> (list: [String], groups: [OpiEmojiGroup])
    ) -> (list: [String], groups: [OpiEmojiGroup]) {
        if let cached = browseCache { return cached }
        let raw = fetch()
        let filtered = filterAndRegroup(raw.list, groups: raw.groups)
        // 原始表空 = 没拿到（留着重试）；过滤后条目变少是正常结果，照常缓存。
        if !raw.list.isEmpty { browseCache = filtered }
        return filtered
    }

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
