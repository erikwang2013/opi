// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器、链接器或语法检查 ——
// InputMethodKit / AppKit 是 Apple 独有框架，在本机 import 就断。
// 本文件**不是**已验证的实现，是给 Mac 开发者的起点。见 macos/README.md
// 的「最不确定的 API」一节：其中有几处是凭记忆写的。
//
// OpiEngine.swift —— OPI 引擎的 C ABI 桥（薄层）。
// 键路由、模式、候选分页全部在 Rust 侧：**键码/状态位表在
// crates/engine-core/src/keys.rs，路由行为与状态在 crates/engine-core/src/router.rs**。
// 本层只做 Swift 值 ↔ C ABI 值的转换 + OpiString 的持有/释放。
// **不要在这里重抄键路由**（审计已点过：路由表曾被抄三份）。
//
// C 符号经 macos/OpiFFI.h 导入（bridging header / module map，见 README「构建集成」）。
//
// 内存所有权（本文件唯一的硬约束）：
//   * opi_* 返回的每个 OpiString 都由 Rust 侧分配（Box<[u16]>），
//     **必须且只能** 用 opi_ffi_free_string 释放恰好一次。
//   * 全模块只有 take(_:) 持有返回的句柄；除它之外不要再写第二次 free。
//     不要用 OpiString.ptr 构造长期存活的 Swift 对象（free 之后即悬垂）。
//   * 传给 Rust 的字符串用 withUTF16；空串传 (nil, 0)
//     （Rust 侧 read_utf16 把 null 当空串/None，不是错误路径）。

import Foundation

// ---------- C ABI 常量镜像（唯一一份；与 crates/tsf-opi/src/logic.rs 对齐） ----------

/// opi_key_event 的 action 码。
enum OpiAction {
    /// 本层不处理，键交系统/客户端（对应 router.rs 的 `KeyAction::PassThrough`）。
    static let unhandled: Int32 = 0
    /// 已消费，无提交文本。**按 router.rs 的契约，收到这个码就要刷新
    /// composition / 候选栏**（它同时覆盖「缓冲变了」与「⇧ 状态机等无变化」两种情况，
    /// C ABI 不区分；多刷一次无害，漏刷会留下过期 preedit）。
    static let handled: Int32 = 1
    /// 提交 text。
    static let commit: Int32 = 2
}

/// 引擎键码空间。**逐条对照 crates/engine-core/src/keys.rs 的常量段抄的**
/// （那是 Apple 两平台的真源；TSF 轨的 `logic.rs` 是另一套编码，**不要**照它抄 ——
/// ⇧/翻页/Delete/空格四个键的编码两边不同）。
/// 可打印字符 = Unicode 码点；特殊键 = specialBase | 低 16 位码。
/// **两段空间不相交**是硬约束：特殊键码永远落不进 ASCII 区，
/// 将来漏了 match 臂也只是「放行给系统」，不会静默变成垃圾字符。
enum OpiKeySpace {
    static let specialBase: UInt32 = 0x1_0000
    static let backspace = specialBase | 0x08 // ASCII BS
    static let tab = specialBase | 0x09 // ASCII HT
    static let ret = specialBase | 0x0d // ASCII CR
    static let escape = specialBase | 0x1b // ASCII ESC
    // 以下四个是 keys.rs 的中立枚举 0x80 起（无通用字符码的键）：
    static let pageUp = specialBase | 0x80
    static let pageDown = specialBase | 0x81
    static let del = specialBase | 0x82 // 向后删除
    static let shift = specialBase | 0x83 // 左右 ⇧ 同码
    // 方向键：router.rs 有显式直通分支（不消费、交系统）。
    static let up = specialBase | 0x84
    static let down = specialBase | 0x85
    static let left = specialBase | 0x86
    static let right = specialBase | 0x87

    /// 空格是**可打印段**：Unicode 码点 0x20，不是特殊键（`keys.rs` 的
    /// `KEY_SPACE` 注释专门写了这件事）。
    /// ⚠️ 别「顺手」写成 specialBase | 0x20：那会落进 `router.rs` 的 `key_event`
    /// 非 ASCII 分支（`_ => PassThrough`），表现为「拼音打一半按空格不提交候选、
    /// 只输出一个空格」。TSF 轨正是 `SPECIAL_BASE | 0x20` 编码，两边不同。
    static let space: UInt32 = 0x20

    /// 取不到字符时的哨兵（= crates/tsf-opi/src/vk.rs 的 `NO_KEYVAL`）。
    /// `router.rs` 的 `key_event` 里 `char::from_u32` 为 None → PassThrough → 键流入应用。
    /// **绝不可回退成平台键码本身**：那是「键码空间与 ASCII 重叠」的重演
    /// （VK_RIGHT=0x27=`'`、小键盘 0x60..0x69=`a`..`i`）。
    static let noKeyval: UInt32 = .max
}

/// opi_key_event 的 states 位。
enum OpiKeyState {
    static let shift: UInt32 = 1 << 0
    static let capsLock: UInt32 = 1 << 1
    static let ctrl: UInt32 = 1 << 2
    static let alt: UInt32 = 1 << 3
    /// macOS 的 Command。常量在 `crates/engine-core/src/keys.rs`，
    /// 直通掩码在 `crates/engine-core/src/router.rs:239`
    /// （`KEY_STATE_CTRL | KEY_STATE_ALT | KEY_STATE_META` → PassThrough），
    /// 所以本层只要如实置位，⌘A/⌘C 就由 Rust 侧放行，不需要 Swift 再拦一道。
    /// ⚠️ 这条依赖要在 Rust 侧改动该掩码时同步复核 —— 掩码一旦去掉 META，
    /// ⌘A 会被当成普通 'a' 吃进拼音缓冲（keys.rs 的 KEY_STATE_META 注释）。
    static let meta: UInt32 = 1 << 4
    static let released: UInt32 = 1 << 26
    static let repeatKey: UInt32 = 1 << 27
    /// 长按。macOS 无来源（同 fcitx5 轨的已知边界），本端恒不置位。
    static let longPressed: UInt32 = 1 << 28
}

/// 模式整数（`opi_switch_mode` / `opi_mode`）。取值 0..=4，与 Rust 侧
/// `api::mode_from_int` / `mode_to_int` 一一对应。
///
/// ⚠️ 五个 case 必须齐：漏了 `traditional` 时 `opi_mode()` 返回 4 会被
/// `OpiMode(rawValue:)` 判为 nil，落到 `?? .pinyin` 的兜底上 —— 繁体模式在 UI 上
/// 显示成拼音，而引擎实际在跑繁体（`switchMode` 传 4 又会被 Rust 正确接受）。
/// 「显示与行为不一致」比「不认识这个模式」难查得多。
///
/// ⚠️ rawValue 是**线上编码**，不是 `Mode` 枚举的声明序 —— 别按声明序「理顺」它。
/// engine-core 的 `Mode`（composer.rs:3）声明序是
/// `Pinyin, Traditional, English, Number, Symbol`，照它推会得到 `traditional = 1`，
/// 与 C ABI 的 4 对不上（`api::mode_from_int` 才是权威）。
enum OpiMode: Int32 {
    case pinyin = 0
    case english = 1
    case number = 2
    case symbol = 3
    case traditional = 4
}

/// `opi_symbol_blocks()` 的 JSON 元素：`[{id,start,end,name,common}]`
/// （serde 固定输出，见 `crates/opi-ffi/src/api/convert.rs` 的 `symbol_blocks_json`）。
///
/// **只含 common 块**：`Engine::symbol_blocks()` 走的是 `symbols.common_blocks()`
/// （`crates/engine-core/src/engine.rs`），非 common 块根本不在这个列表里 ——
/// 所以 `common` 字段在本出口上恒为 true，别拿它当「全部块」用。
///
/// ⚠️ **id 的类型在两端不一样**（这是本出口唯一的陷阱）：
/// Rust 侧是 `symbols::BlockId(pub u16)`，JSON 里就是 0..=65535；
/// 而 C ABI 的 `opi_symbols_in_block(int16_t)` 收**有符号** 16 位 ——
/// 两者只能对齐到 0..=32767。当前数据表 `data/raw/symbol_blocks.tsv` 的 id 是
/// 1..6，够用；但若把 u16 直接按 `Int16` 解码，id > 32767 时 `JSONDecoder`
/// 会**抛异常**，而这里对解析失败的处理是返回空数组 —— 后果是
/// 「符号面板整块空掉、没有任何日志」。所以按 `Int` 解码、显式收窄、
/// 收不下就 NSLog 跳过（不静默，与 `mode()` 的兜底同一条纪律）。
struct OpiSymbolBlock: Decodable {
    /// Rust 的 `BlockId(u16)`，按 Int 收（见上面的类型说明）。
    let id: Int
    /// Unicode 区块起止（u32 码点，闭区间）。
    let start: Int
    let end: Int
    let name: String
    let common: Bool

    /// 收窄到 C ABI 的 `int16_t`。收不下（id > 32767）→ nil。
    var abiId: Int16? {
        let narrowed = Int16(exactly: id)
        if narrowed == nil { NSLog("opi: 符号块 id %d 超出 C ABI 的 int16_t，该块不可查询", id) }
        return narrowed
    }
}

// ---------- OpiString 持有/释放（全模块唯一出口） ----------

/// 取走 OpiString 的内容并释放句柄。**每个返回句柄恰好调用一次**。
func take(_ s: OpiString) -> String {
    defer { opi_ffi_free_string(s) } // defer：即使下面提前 return 也一定释放
    guard let ptr = s.ptr, s.len > 0 else { return "" } // NULL = 空串哨兵
    return String(decoding: UnsafeBufferPointer(start: ptr, count: s.len), as: UTF16.self)
}

/// 把 Swift 字符串借给 Rust（ptr + len，UTF-16）。空串传 (nil, 0)。
func withUTF16<R>(_ s: String, _ body: (UnsafePointer<UInt16>?, Int) -> R) -> R {
    let units = Array(s.utf16)
    if units.isEmpty { return body(nil, 0) }
    return units.withUnsafeBufferPointer { body($0.baseAddress, $0.count) }
}

/// JSON 文本数组 → Swift 数组。解析失败按空数组处理（Rust 侧恒返回合法 JSON）。
func takeTexts(_ s: OpiString) -> [String] {
    let raw = take(s)
    guard let data = raw.data(using: .utf8),
          let arr = try? JSONSerialization.jsonObject(with: data) as? [String]
    else { return [] }
    return arr
}

// ---------- 引擎单例 ----------

/// 进程级单例：C ABI 的引擎状态本身就是进程级（Rust 侧 `api::SINGLETON`），
/// 这里只是把调用收口到一处。
final class OpiEngine {
    static let shared = OpiEngine()

    private init() {}

    /// 词库默认路径：~/Library/Application Support/opi/luna.opid
    /// （镜像 fcitx5 轨的 $XDG_DATA_HOME/opi/luna.opid 与 Android 的
    /// EngineLoader.FILE_NAME）。⚠️ 这条路径约定在 macOS 上未经核对。
    static var defaultDictionaryPath: String {
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory())
        return base.appendingPathComponent("opi/luna.opid").path
    }

    /// 装载词库。**返回值必须接** —— 这是 fcitx5 轨修过的一个静默失效：
    /// `opi_load(坏路径)` 返回 false 且 Rust 单例**保持为空**，于是所有 opi_* 出口
    /// 退化成空操作，表现为「输入法已装好、按键被吃、一个字都不出，任何地方没有错误」。
    /// 回退必须由本层做（Rust 侧 `install(坏路径) -> Err` 是有意语义，不要改）。
    /// 参见 crates/fcitx5-opi/cpp/opi_fcitx5.cpp 的 loadDictionary() 长注释。
    @discardableResult
    func loadDictionary(path: String? = nil) -> Bool {
        let path = path ?? OpiEngine.defaultDictionaryPath
        if withUTF16(path, { opi_load($0, $1) }) { return true }

        // 能走到这里就是「装不上」。必须留日志：否则「词库损坏」与「本来就没装词库」
        // 在用户侧无法区分，只表现为候选质量骤降。
        if FileManager.default.fileExists(atPath: path) {
            NSLog("opi: 词库 %@ 装载失败，回退内置词库", path)
        } else {
            NSLog("opi: 未找到词库 %@，使用内置回退词库（候选质量会明显下降）", path)
        }
        let ok = withUTF16("", { opi_load($0, $1) }) // 空串 → 内置回退词库
        if !ok { NSLog("opi: 内置回退词库也装载失败 —— 引擎不可用") }
        return ok
    }

    /// 键事件路由（唯一入口）。返回 (action, 需要提交的文本)。
    /// action != OpiAction.commit 时 text 恒为空串（但仍会走 take 释放）。
    func keyEvent(keyval: UInt32, states: UInt32) -> (action: Int32, text: String) {
        let r = opi_key_event(keyval, states)
        return (r.action, take(r.text))
    }

    func buffer() -> String { take(opi_buffer()) }

    func candidates(limit: Int = 64) -> [String] { takeTexts(opi_candidates(limit)) }

    /// 全局索引选词（与 `candidates(limit:)` 的数组同序）。返回需提交的文本。
    func select(index: Int) -> String { take(opi_select(index)) }

    func clear() { opi_clear() }

    func mode() -> OpiMode { OpiMode(rawValue: opi_mode()) ?? .pinyin }

    /// 切换模式。入参只可能是 0..=4（枚举封死了）；Rust 侧对越界值是**静默不动作**
    /// （`api::mode_from_int` 返回 None → 什么都不做、无返回值可查），
    /// 所以将来若从菜单/配置读到裸整数，**必须先自己校验**再调。
    func switchMode(_ mode: OpiMode) { opi_switch_mode(mode.rawValue) }

    // ---------- 全角 / 符号开关（三个出口 2026-09-27 落地，同日声明面补齐） ----------

    /// 全角 ⇄ 半角，返回**切换后的新状态**。
    /// 故意**不加** `@discardableResult`：返回值就是这条出口存在的理由。
    func toggleFullwidth() -> Bool { opi_toggle_fullwidth() }

    /// 全角开关的**读侧**（与 `shiftState()` / `mode()` 同构）。
    ///
    /// ⚠️ 三条硬约定**写在声明头里**（`macos/OpiFFI.h` 的「全角 / 符号开关」一节），
    /// 此处**不重抄** —— 这个仓库被「同一语义抄多份」坑过多次，Rust 侧与头文件才是真源。
    /// 只记两条 Swift 侧才有的：
    ///   * 它给**状态显示**用，**别**拿去推「这个键在全角下该出什么」（映射不是纯函数）。
    ///   * 本目录**还没有全角指示**（菜单项也没有），所以暂时没有陈旧值可清。
    func fullwidthState() -> Bool { opi_fullwidth_state() }

    /// 符号面板开关：**Pinyin ⇄ Symbol 来回切**（不是「切到 Symbol」）。
    /// 返回**待上屏文本**（空串 = 无提交）。约定与出处见声明头，此处不重抄。
    func toggleSymbol() -> String { take(opi_toggle_symbol()) }

    // ---------- 符号库（面板数据） ----------
    //
    // 三个出口都是**只读查询**：不碰缓冲、不改页码、不参与键路由，所以可以
    // 在任意时刻调用（面板打开时查一次即可）。
    // ⚠️ 符号面板**不要**把符号键送回 `opi_key_event`：引擎的标点表会把 `,` 之类
    // 改写成中文标点/全角（那是**文本**模式的行为），而用户在面板上点的 `,` 就是
    // 要 `,`。Android 的 `SymbolPanel` 同样绕过引擎直提（`router::commitText`）。

    /// 常用符号块（**只有 common 块**，见 `OpiSymbolBlock` 的注释）。
    /// JSON 解析失败 → 空数组（面板空着，不崩）。引擎未装载 → 空数组。
    func symbolBlocks() -> [OpiSymbolBlock] {
        let raw = take(opi_symbol_blocks())
        guard let data = raw.data(using: .utf8),
              let list = try? JSONDecoder().decode([OpiSymbolBlock].self, from: data)
        else {
            // Rust 侧恒返回合法 JSON；走到这里说明两端契约漂了，别说成「没有符号」。
            if !raw.isEmpty { NSLog("opi: symbol_blocks JSON 解码失败：%@", raw) }
            return []
        }
        return list
    }

    /// 某个块里的符号（JSON 文本数组）。
    /// 传 `OpiSymbolBlock.abiId`；nil（id 越界）→ 空数组。
    /// ⚠️ 负 id 在 Rust 侧按**越界**处理（空数组，`cabi.rs` 的 `opi_symbols_in_block`），
    /// 不会钳成块 0。
    func symbolsInBlock(id: Int16) -> [String] {
        guard id >= 0 else { return [] }
        return takeTexts(opi_symbols_in_block(id))
    }

    /// 关键字搜索符号（JSON 文本数组）。
    /// ⚠️ **空关键字返回全部条目**（`symbols::search("")`），不是「什么都没搜到」——
    /// Android 的 `SymbolCatalog.all` 正是靠这个语义拿「全部」。
    func searchSymbols(keyword: String) -> [String] {
        withUTF16(keyword) { p, n in takeTexts(opi_search_symbols(p, n)) }
    }
}
