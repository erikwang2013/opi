// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器 —— 一个字符都没有编译过。
// 见 ios/README.md「在 Mac 上第一件要做的事」：先让它编译过，再谈功能。
//
// `OpiEngine` 的**方案族**包装（方案轴：按键 → 字母类）。
// 为什么单开一个文件：`OpiEngine.swift` 已到 496 行、顶在本仓 500 行源码上限上；
// 拆法照抄 Rust 侧的族边界 —— `crates/opi-ffi/src/cabi_scheme.rs`（方案）+ `cabi_wubi.rs`
// （五笔旋钮），见 `OpiEngineWubi.swift` 的头注。
// **契约面（唯一真源）：`macos/OpiFFI.h`**。`ios/OpiFFI.h` 只是转发头
// （`#include "../macos/OpiFFI.h"`），本文件**不**重抄任何 C 声明。
//
// ⚠️ 与 `OpiEngine.swift` 的既有注释同一条纪律：这里的每个方法都**未编译**、
// 每个哨兵值都只对着 Rust 源码读过一遍。Mac 上第一件事是核签名，不是调行为。

import Foundation

/// 输入方案（引擎轴之一：按键 → 字母类）。**与语言模式 `OpiMode` 正交** ——
/// 「拼音 9 键 + 英文」是两个轴各取一个值，不是同一根轴上的两个点。
///
/// 数值真源 = `crates/engine-core/src/scheme.rs` 的 `Scheme::code()`：
/// 全拼 0 / 拼音9键 1 / 拼音14键 2 / 笔画 3 / 双拼 4 / 五笔 5。
///
/// ⚠️ **头文件那句已经漂了**（2026-10-14 实读）：`macos/OpiFFI.h` 的 `opi_set_scheme`
/// 注释写的是「0=全键盘 1=拼音9键 2=拼音14键 3=笔画 4=双拼，越界忽略」—— **漏了 5=五笔**
/// （五笔是同一天 M4 加的）。与 `opi_switch_mode` 漏 4=Traditional 是同一类**文档**笔误，
/// 行为以 Rust 的 `code()` / `from_code` 为准。别照那句注释写校验。
/// **不要照 Rust 的枚举声明顺序编号** —— 与 `OpiMode` 同一个坑（那边 Traditional
/// 声明在第 2 位、编码却是 4）：这里每个 case 显式写死数值，`from_code` 的
/// 越界语义是 `None`（不是钳制）。
///
/// ⚠️ **本枚举不可用 `init(rawValue:)` 的 `!` 强解**：`opi_scheme()` 在**未装载**
/// 时返回 0（= `.full` 哨兵）—— 与「真的装的是全拼」**不可区分**。这不是本层能修的
/// 事（要修得动 C ABI 的读侧，见 ios/README.md 的缺口一节）；本层能做的是：
/// 取值只信「用户自己设过的值」，别拿 `scheme()` 当「引擎现在真的是什么」的判据。
enum OpiScheme: Int32 {
    /// 全拼（26 键）。也是 `opi_scheme()` 未装载时的哨兵值。
    case full = 0
    /// 拼音 9 键（T9）：数字键 2..9 收进缓冲，按 `scheme.rs` 的 Nine 类表展开。
    case nine = 1
    /// 拼音 14 键：两字母一键，发的是字母对的**首个**字母（`qw→q`）。
    case fourteen = 2
    /// 笔画：键值是 `h/s/p/n/z`（横竖撇捺折），换的是**词典**（stroke.opid）。
    case stroke = 3
    /// 双拼：键面就是 26 键，解码在引擎侧（布局 0..5 见 `setShuangpinLayout`）。
    case shuangpin = 4
    /// 五笔：键面同样是 26 键，码表另装（见 `OpiEngineWubi.swift`）。
    case wubi = 5
}

extension OpiEngine {

    // MARK: - 方案（`cabi_scheme.rs` 族）

    /// 当前方案（`opi_scheme()`）。**未装载 → 0（= .full）**，见 `OpiScheme` 的注释。
    ///
    /// ⚠️ 未知编码**不静默**兜底 —— 与 `mode()` / `shiftState()` 同一条纪律：
    /// 兜成 `.full` 会让「方案漂移」表现成「9 键盘画成了 26 键」这种**看似 UI 的 bug**，
    /// 有日志才查得到。未知只可能来自「Rust 加了第七个方案而这里没跟上」。
    func scheme() -> OpiScheme {
        let raw = opi_scheme()
        guard let s = OpiScheme(rawValue: raw) else {
            NSLog("[OPI] 未知方案编码 %d —— 两端方案表已漂移，按全拼处理", raw)
            return .full
        }
        return s
    }

    /// 换方案（`opi_set_scheme`）。
    ///
    /// ⚠️ **换方案会清缓冲**（Rust 侧语义，与 `switch_language` 的「保缓冲」刻意相反）：
    /// 9 键缓冲里是数字、26 键缓冲里是字母，跨方案保留毫无意义。
    /// 所以调用方**不需要**先自己 `clear()`；反过来，**不要**指望换完还能拿到
    /// 换之前打的拼音 —— 那段东西到这里就没了（Android 在换方案前先
    /// `commitPending*` 提交，本端没有那条出口，见 ios/README.md 的缺口一节）。
    ///
    /// ⚠️ 入参已封死在 0..=5（枚举），当前**安全**；若将来从偏好设置/持久化读进
    /// **裸整数**，必须先自校验 —— 越界在 Rust 侧是静默不动作，且函数返回 void，
    /// 调用方查不到失败（症状：点了没反应、无日志）。
    func setScheme(_ s: OpiScheme) { opi_set_scheme(s.rawValue) }

    // MARK: - 双拼布局

    /// 双拼布局下标（`opi_shuangpin_layout()`）。0=N 自然码 … 见桌面/Android 的
    /// 「双拼布局」节。未装载 → 0。
    ///
    /// ⚠️ **名称表不在本层**：C ABI 与 JNI **都没有**「布局名」出口（名字表在
    /// Android `ImePreferences.kt` 与桌面设置页各有一份，且有锁步门禁）——
    /// iOS 这边**没有**设置界面，所以本层只包读数/写数，不造第三份名字表。
    /// 将来做设置界面时，名字表要跟着门禁走，别手抄一串中文。
    func shuangpinLayout() -> Int { Int(opi_shuangpin_layout()) }

    /// 设双拼布局下标（`opi_set_shuangpin_layout`，**收 Int**）。
    ///
    /// 三条写侧约定（Rust 侧语义，读一遍 `cabi_scheme.rs` 就能看到）：
    ///   * 负数 → **钳到 0**（不是忽略、不是报错）；
    ///   * 过大的值 → **钳到最后一个布局**（同样不报错）；
    ///   * **不清缓冲**（与 `setScheme` 相反 —— 布局是解码表的换法，缓冲仍是拼音串）。
    /// 所以本层**不做**越界判断（钳制是引擎的契约，本层再判一次就是抄第二份）；
    /// 只做 Swift 侧的类型收窄：`Int` → `Int32` 溢出的强转会**运行时崩**。
    func setShuangpinLayout(_ index: Int) {
        guard let v = Int32(exactly: index) else {
            NSLog("[OPI] 双拼布局下标 %ld 超出 Int32 —— 已忽略（引擎侧的钳制只处理值域内越界）", index)
            return
        }
        opi_set_shuangpin_layout(v)
    }

    // MARK: - 笔画词典

    /// 装笔画词典（`opi_load_stroke`，2026-10-09 落的出口）。
    ///
    /// 与 `loadTrad(path:)` 同一条形状：**可选增强** —— 失败只让笔画方案回退普通
    /// 前缀路（候选变少/为空），**不碰**已装好的主词库。返回 false 时调用方应记日志，
    /// 但**不必**当成致命错误（Android `EngineLoader.loadStrokeAsset` 同款语义）。
    @discardableResult
    func loadStroke(path: String) -> Bool {
        OpiEngine.withUTF16(path) { p, n in opi_load_stroke(p, n) }
    }
}
