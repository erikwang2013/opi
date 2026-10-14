// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器 —— 一个字符都没有编译过。
// 见 ios/README.md「在 Mac 上第一件要做的事」：先让它编译过，再谈功能。
//
// `OpiEngine` 的**五笔旋钮族**包装（2026-10-14，M4）。单开一个文件的两个理由：
//   1. `OpiEngine.swift` 已到 496 行、顶在 500 行源码上限；
//   2. 拆法照抄 Rust 侧的族边界（`crates/opi-ffi/src/cabi_wubi.rs` 与 `cabi_scheme.rs`
//      分列）—— 本仓的纪律是「族边界两端一致」，不是随手切。
// **契约面（唯一真源）：`macos/OpiFFI.h` 的「五笔旋钮族」一节**，本文件不重抄 C 声明。
//
// 六个旋钮的**共同三条**（读侧/写侧，Rust 侧注释写死、这里只记不复述理由）：
//   * 写侧**不清缓冲**（与 `setScheme` 的「清缓冲」刻意相反）；
//   * 三条枚举旋钮的写侧**越界忽略**（不是钳、不是报错，返回 void 查不到）；
//   * **读侧在引擎未装载时返回哨兵**（枚举 → 0、bool → false），与「用户选的就是
//     那个值」**不可区分** —— 所以别拿读侧当「引擎现状」的判据。
//
// ⚠️ 本族**没有任何 iOS 入口**（2026-10-14 现状）：iOS 侧还没有设置界面，
// 加进来是为了「出口面先齐」（Android 的五个旋钮在设置页的「五笔」节里）。
// 谁先做 iOS 设置页，谁就把这些接到 UI 上 —— 但也**别忘**：五笔方案要能用，
// 除了旋钮还得 `loadWubi(path:)` + `setScheme(.wubi)`（头注：装载**不**换方案）。

import Foundation

extension OpiEngine {

    // MARK: - 码表

    /// 装五笔码表（`opi_load_wubi`，86/98 各一张）。
    ///
    /// **按方案装一张**，别两张都装：两表码空间重叠、权重不可比，同时进引擎就是
    /// 两套体系打架（Android `EngineLoader` 与 fcitx5 的 `opi_wubi.h` 同一条裁决）。
    /// 返回 false（空/坏路径/引擎未装载）时方案回退普通路径 —— 是**可选增强**的失败，
    /// 不是致命错误（同 `loadStroke`）。
    ///
    /// ⚠️ **装载不换方案**：装完还得 `setScheme(.wubi)`，否则键盘照旧走全拼
    /// （头文件第 91 行明写；这是「装成功但没用上」的静默坑，调用方要自己接住）。
    @discardableResult
    func loadWubi(path: String) -> Bool {
        OpiEngine.withUTF16(path) { p, n in opi_load_wubi(p, n) }
    }

    // MARK: - 三个枚举旋钮（0/1 二值；越界写忽略、未装载读 → 0 哨兵）

    /// 风格：0=纯五笔 1=五笔拼音混输（`WubiStyle`）。未装载 → 0（哨兵）。
    /// ⚠️ 读到的 0 **不蕴含**「用户选了纯五笔」—— 引擎没装载时也返 0。
    func wubiStyle() -> Int32 { opi_wubi_style() }

    /// 设风格。**越界忽略**（不钳、不报错、无返回值可查）：本层收 `Int32` 裸值，
    /// 若将来从偏好设置读进字符串/JSON，先自己校验再传（值域 0..=1）。
    func setWubiStyle(_ style: Int32) { opi_set_wubi_style(style) }

    /// 混入方式：0=直接字母 1=z键（`WubiMixedEntry`）。未装载 → 0（哨兵）。
    func wubiMixedEntry() -> Int32 { opi_wubi_mixed_entry() }

    /// 设混入方式（值域 0..=1）。**越界忽略**，同 `setWubiStyle`。
    func setWubiMixedEntry(_ entry: Int32) { opi_set_wubi_mixed_entry(entry) }

    /// 回车风格：0=清空 1=上屏原文（`WubiReturnStyle`）。未装载 → 0（哨兵）。
    ///
    /// ⚠️ **2026-10-14 订正**：本节原写「消费端四端都还没接」，**已过期，别照旧读**。
    /// 判据的单一实现在 `engine_core::Engine::enter_action`（`engine_enter.rs`）。
    /// 当天 23:20 前后实读消费面：C ABI 键路由（`router_edit_keys.rs::handle_enter`
    /// —— **iOS 走的正是这条**）、fcitx5、TSF 已接；Android `KeyRouter.handleEnter`
    /// 实读**仍是**写死的 `select(0)`（未接）。⇒ 本值在 iOS 上**确实**决定回车行为
    /// （前提：当前 scheme 为五笔）。缺省档 = 清空（Sogou 语义）：装了五笔表、选了
    /// 五笔模式之后，打一半按回车**不上屏、直接清掉** —— 本端没有设置界面，改不了这一档。
    func wubiReturnStyle() -> Int32 { opi_wubi_return_style() }

    /// 设回车风格（值域 0..=1）。**越界忽略**，同 `setWubiStyle`。
    func setWubiReturnStyle(_ style: Int32) { opi_set_wubi_return_style(style) }

    // MARK: - 个数（钳区间，唯一一个不是「越界忽略」的）

    /// 候选个数上限（1..=9）。**写侧钳区间**（不是忽略）：
    /// `WUBI_CANDIDATE_LIMIT_DEFAULT` = 5、`WUBI_CANDIDATE_LIMIT_MAX` = 9
    /// （`engine_wubi.rs` 的常数，被门禁钉住的那种）。
    /// **未装载 → 0 是哨兵**（0 不是合法值域，所以能当哨兵用 —— 但别把它显示成
    /// 「上限 0」）。本层只做 Swift 侧的类型收窄。
    func wubiCandidateLimit() -> Int32 { opi_wubi_candidate_limit() }

    /// 设候选个数上限。**钳区间**：<1 → 1，>9 → 9（所以传 0 得到的是 1，不是哨兵）。
    func setWubiCandidateLimit(_ n: Int32) { opi_set_wubi_candidate_limit(n) }

    // MARK: - 自动上屏（bool）

    /// 自动上屏。**缺省 true**；**未装载 → false 是哨兵** ——
    /// 注意这条与「缺省 true」方向相反：读侧 false 可能是「引擎没装载」，
    /// 别直接当成「用户关掉了自动上屏」。
    func wubiAutoCommit() -> Bool { opi_wubi_auto_commit() }

    /// 设自动上屏。bool 无越界问题，直接转发。
    func setWubiAutoCommit(_ on: Bool) { opi_set_wubi_auto_commit(on) }

    // MARK: - 刻意**没有**包的出口
    //
    // `opi_wubi_next_keys()`（编码提示）：C 声明有、头注带性能警告
    // （每次调用全量重算 O(26×前缀查询)，缓冲变化时取一次，别每帧调）。
    // 本层**不包**它 —— iOS 这边**没有**任何 UI 会显示编码提示（26 键五笔布局
    // 用的是 `ModeKeyTables.letterRows()`，没有提示条）。包一个没有调用方的出口只会让下一个人
    // 以为「有 UI 在用」。要做提示条时再包，那时把「缓冲变化时才取」写进调用点。
    //
    // `opi_enter_action()`（回车该做什么：0=选首候选 1=清空缓冲 2=上屏原文；2026-10-14
    // 当天落地，出口在 `cabi_wubi.rs`、声明在 `macos/OpiFFI.h`）：本层**同样不包** ——
    // iOS 的回车走 `opi_key_event`，键路由（`router_edit_keys.rs::handle_enter`）里问的
    // 就是同一条 `Engine::enter_action`，已在覆盖范围内；本端**没有**第二条通道会问
    // 「回车该做什么」。包一个没有调用方的出口 = 让人以为有条独立的路（同上）。
}
