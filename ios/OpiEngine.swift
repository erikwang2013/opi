// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器 —— 一个字符都没有编译过。
// 语法错误、类型错误、API 名字写错都属于**预期内**情况，见 ios/README.md
// 「在 Mac 上第一件要做的事」：先让它编译过，再谈功能。
//
// OPI C ABI 的 Swift 薄桥。
//
// 设计原则（本轮定死，改动请先读这条）：
//   1. **薄**。这里只做「Swift 类型 ←→ C ABI 类型」的转换，不实现任何输入法逻辑。
//      键路由在 Rust（crates/engine-core/src/router.rs，经 opi_key_event 暴露）。
//      本项目已被「同一张路由表抄三份」坑过 —— 不要在这里重抄一份分流逻辑。
//   2. **键事件优先走 opi_key_event**。本文件另有 load / select / selectPage /
//      switchMode / 用户词导入导出等**直通包装**，它们是备用出口（调试、以及用户词
//      管理这类 key_event 覆盖不到的场景）—— KeyboardViewController 的**按键**
//      正常路径不该用它们。
//      ⚠️ 别把不存在的东西写进来（本文件曾经写错）：`opi_input_key` / `opi_backspace`
//      （单字符入口，已被 key_event 取代）、`opi_candidates(limit)`（全局不分页，
//      本文件用的是 `candidatesPage()`）、`opi_shift_state()`（见
//      KeyboardLayout.swift 的注释）在这里**都没有包装**。要加的话先读
//      ios/README.md 的「缺的出口」一节。
//   3. **所有权写死在注释里**：OpiString 由 Rust 分配，必须由
//      opi_ffi_free_string 释放恰好一次。本文件所有取值路径都收口到
//      takeString(_:)，只有那里允许调用 free —— 新增出口请复用它，不要自己写
//      String(decoding:) 然后忘记 free（这是最容易漏且会持续泄漏的地方）。

import Foundation

/// opi_key_event 返回的 action：0=未处理（交系统）1=已处理 2=提交。
/// 对应 Rust `engine_core::router::KeyAction`（PassThrough / EngineHandled / Input）。
enum OpiKeyAction {
    /// 交系统处理该键（空缓冲的退格/回车、Ctrl/Alt/⌘ 组合、Tab/Esc、方向键等）。
    case passThrough
    /// 已被引擎消费，无提交文本；调用方需刷新 composition（拼音缓冲）与候选栏。
    case handled
    /// 提交文本到客户端。
    case commit(String)
}

/// 输入模式。
///
/// **真源是 `crates/opi-ffi/src/api/mod.rs` 的 `mode_to_int` / `mode_from_int`**
/// （C ABI 与 JNI 共用），**不是** Rust `composer::Mode` 枚举的**声明顺序**——
/// 两者**不同**：枚举声明是 Pinyin, Traditional, English, Number, Symbol，
/// 若照声明顺序编号，Traditional 会是 1；而 C ABI 里它是 **4**。
///
/// 所以这里**每个 case 都显式写死数值**，不依赖 Swift 的声明顺序推导。
/// 这个坑真实发生过：漏掉 `traditional = 4` 时，`opi_mode()` 返回 4
/// 会让 `OpiMode(rawValue:)` 得到 nil，再被 `?? .pinyin` 兜成拼音 ——
/// **繁体模式被静默显示成拼音**（没有崩溃、没有日志，最难查的那种）。
enum OpiMode: Int32 {
    case pinyin = 0
    case english = 1
    case number = 2
    case symbol = 3
    /// 繁体模式。行为同拼音（小写入缓冲、⇧ 无效），引擎层路由到 trad 词典。
    /// 模式入口见 `KeyboardFunctionKey.cycleMode`（中→繁→英→中，对齐 Android
    /// `ImeScreen.kt` 的 `toggleMode()`）。
    case traditional = 4
}

/// 前端 ⇧ 三态（`opi_shift_state()`）：0=OFF 1=SINGLE 2=LOCK。
///
/// **与 `OpiKey.stateShift` 不是一回事**：那个是**引擎侧** shift 位（`opi_set_shift`
/// 打的是同一个），而三态是**前端**状态 —— 英文直传路径的大小写由它决定，
/// 引擎位看不出来。所以 ⇧ 的高亮（尤其 Single 与 Lock 的区别）只能读这里。
///
/// ⚠️ 本枚举**目前没有调用点**：`KeyboardLayout` 故意不画 ⇧ 高亮（见那里的长注释），
/// 这里先把出口包出来（README「缺的出口：已落地」第 4 条点名的两步中的第 ① 步）。
/// 第 ② 步（让 UI 读它）与「先过编译」的排序不变。
enum OpiShiftState: Int32 {
    case off = 0
    /// 下个字母大写后**引擎会自动复位**（`KeyRouter` 的三态状态机），
    /// 所以高亮不能缓存 —— 每次刷新都重读出口。
    case single = 1
    case lock = 2
}

/// `opi_symbol_blocks()` 的 JSON 元素：`[{id,start,end,name,common}]`
/// （serde 固定输出，见 `crates/opi-ffi/src/api/convert.rs` 的 `symbol_blocks_json`）。
///
/// **只含 common 块**：`Engine::symbol_blocks()` 走的是 `symbols.common_blocks()`，
/// 非 common 块不在这个列表里 —— `common` 字段在本出口上恒为 true。
///
/// ⚠️ **id 的类型两端不一样**：Rust 侧是 `symbols::BlockId(pub u16)`（JSON 里 0..=65535），
/// 而 C ABI 的 `opi_symbols_in_block(int16_t)` 收**有符号** 16 位 —— 只能对齐到
/// 0..=32767。当前 `data/raw/symbol_blocks.tsv` 的 id 是 1..6，够用；但把 u16 直接按
/// `Int16` 解码会在 id > 32767 时**抛异常**，而本层对解析失败的处理是返回空数组 ——
/// 后果是「符号面板整块空掉、无任何日志」。故按 `Int` 收、显式收窄、收不下就记日志。
struct OpiSymbolBlock: Decodable {
    let id: Int
    let start: Int
    let end: Int
    let name: String
    let common: Bool

    /// 收窄到 C ABI 的 `int16_t`；id > 32767 → nil（并记日志，不静默）。
    var abiId: Int16? {
        let narrowed = Int16(exactly: id)
        if narrowed == nil {
            NSLog("[OPI] 符号块 id %d 超出 C ABI 的 int16_t，该块不可查询", id)
        }
        return narrowed
    }
}

/// 键码常量，**唯一真源是 crates/engine-core/src/keys.rs**。
///
/// 为什么 Swift 侧还是得有一份：`keys.rs` 只认「Unicode 码点 / SPECIAL_BASE|低16位」
/// 这一种编码，平台键码 → 该编码的映射必须由胶水做（`keys.rs` 模块头
/// 「Apple 侧的键位来源」一节明确这么分工）。这里是**编码常量**，不是路由表；
/// 路由表（哪个键干什么）在 crates/engine-core/src/router.rs，仍然只存在于 Rust。
///
/// 改动时：先改 keys.rs，再同步这里。不要只改一边。
enum OpiKey {
    /// 特殊键基址。低 16 位取值见下方各常量。
    static let specialBase: UInt32 = 0x1_0000

    // 与 keys.rs 的 KEY_* 逐条对应（四大控制键取 ASCII 控制码，其余取中立枚举）。
    static let backspace: UInt32 = specialBase | 0x08
    static let tab: UInt32 = specialBase | 0x09
    static let ret: UInt32 = specialBase | 0x0D
    static let escape: UInt32 = specialBase | 0x1B
    static let pageUp: UInt32 = specialBase | 0x80
    static let pageDown: UInt32 = specialBase | 0x81
    static let delete: UInt32 = specialBase | 0x82
    static let shift: UInt32 = specialBase | 0x83
    static let up: UInt32 = specialBase | 0x84
    static let down: UInt32 = specialBase | 0x85
    static let left: UInt32 = specialBase | 0x86
    static let right: UInt32 = specialBase | 0x87

    /// 空格是**可打印段**（Unicode 码点 0x20），不是特殊键 —— 与 TSF 轨的编码不同。
    static let space: UInt32 = 0x20

    // ---- 键状态位（与 keys.rs 的 KEY_STATE_* 逐条对应）----
    static let stateShift: UInt32 = 1 << 0
    /// 位布局的一部分；定义了但路由不用它（大小写由码点 + ⇧ 状态机决定）。
    static let stateCapsLock: UInt32 = 1 << 1
    static let stateCtrl: UInt32 = 1 << 2
    static let stateAlt: UInt32 = 1 << 3
    /// macOS 的 ⌘ / iOS 外接键盘的 Command。并入直通掩码，否则 ⌘A 会把 'a' 吃进拼音缓冲。
    static let stateMeta: UInt32 = 1 << 4
    static let stateReleased: UInt32 = 1 << 26
    static let stateRepeat: UInt32 = 1 << 27
    /// ⇧ 长按 = Lock（对应 KeyRouter 的三态状态机）。
    static let stateLongPressed: UInt32 = 1 << 28

    /// 可打印字符 → keyval（就是它自己的 Unicode 码点）。
    static func printable(_ scalar: Unicode.Scalar) -> UInt32 { UInt32(scalar.value) }
}

/// C ABI 薄桥。**非线程安全假设**：Rust 侧有 Mutex 保护单例，但 UI 调用一律回主线程
/// （键盘扩展里所有 proxy 操作本来就必须在主线程），所以这里不做加锁。
final class OpiEngine {

    // MARK: - 字符串所有权（唯一允许 free 的地方）

    /// 把 Rust 分配的 OpiString 取成 Swift String，并**释放句柄**。
    ///
    /// 所有权契约（crates/opi-ffi/src/cabi.rs）：
    ///   - OpiString 的缓冲由 Rust `Box<[u16]>` 分配，`len` 是 u16 个数，
    ///     **不是 NUL 结尾**的 C 字符串 —— 不要传给 `String(cString:)`。
    ///   - `ptr == nil` 是「空串哨兵」，不是错误；此时 len 必为 0。
    ///   - 释放必须恰好一次，`opi_ffi_free_string` 对空句柄是 no-op，
    ///     所以**无条件释放**是安全的（Rust 侧对 action != 2 也返回空句柄，
    ///     注释明说「调用方可以无条件释放」）。
    ///   - 释放后 ptr 立即失效，任何持有它的 UnsafePointer 都变悬垂 ——
    ///     所以这里先把内容拷进 String，再 free，顺序不能反。
    static func takeString(_ s: OpiString) -> String {
        defer { opi_ffi_free_string(s) }   // defer：任何 return 路径都释放，包括下面提前 return
        guard let ptr = s.ptr, s.len > 0 else { return "" }
        let units = UnsafeBufferPointer(start: ptr, count: s.len)
        // decoding:as: 对孤立代理项产出替换字符而非崩溃 —— 与 Rust 侧
        // String::from_utf16 的失败语义相称（我们从不产生孤立代理项，这只是兜底）。
        return String(decoding: units, as: UTF16.self)
    }

    /// 把 Swift String 借给需要 (const uint16_t*, size_t) 的 C 函数。
    ///
    /// 空串传 (nil, 0)：Rust `read_utf16` 对 null 返回 None，各出口对 None 的处理
    /// 与空串**不总是相同**（例：`opi_load` 的 null → 内置回退词库；
    /// `opi_import_user_words` 的 null → 返回 -1 失败）。传什么由调用方决定：
    /// 这里只忠实传递「空 → nil」，需要区分「真空串」与「没给」的出口请自己判断。
    ///
    /// ⚠️ 未编译：`withUnsafeBufferPointer` 在空数组上 baseAddress 为 nil，
    /// 所以用了 Optional 指针 —— 这一点我按记忆写，Mac 上请确认类型对得上
    /// （C 声明是 `const uint16_t *`，Swift 侧通常导入为 `UnsafePointer<UInt16>?`）。
    static func withUTF16<R>(_ s: String, _ body: (UnsafePointer<UInt16>?, Int) -> R) -> R {
        let units = Array(s.utf16)
        return units.withUnsafeBufferPointer { buf in
            body(buf.baseAddress, buf.count)
        }
    }

    // MARK: - 键事件（正常路径）

    /// 键事件路由。**键盘的正常输入全部走这里**，不要自己调 opi_input_key。
    ///
    /// - Parameters:
    ///   - keyval: 可打印字符 = Unicode 码点；特殊键 = `OpiKey.specialBase | 低16位`
    ///   - states: `OpiKey.state*` 的位或
    ///
    /// 按下与**抬起都必须调用**：router.rs 对可打印键「抬起按按下的结论回复」
    /// （last_printable 单槽），只发按下会让 ⇧ 状态机与 keyup 语义不完整。
    /// 抬起事件永远不会返回 .commit。
    func keyEvent(keyval: UInt32, states: UInt32) -> OpiKeyAction {
        let r = opi_key_event(keyval, states)
        // text 无论 action 取值都必须取走并释放 —— action != 2 时它是空句柄，
        // takeString 里的 free 是 no-op。先取再判，避免遗漏释放。
        let text = OpiEngine.takeString(r.text)
        switch r.action {
        case 2:  return .commit(text)
        case 1:  return .handled
        default: return .passThrough   // 0 以及任何未来新增的未知 action
        }
    }

    // MARK: - 装载

    /// 装载主词库。返回 false 时**所有出口都会退化成空操作**（见 OpiFFI.h 注释），
    /// 调用方必须接住这个返回值并给出可见的失败反馈（否则表现为「键盘完全没反应」，
    /// fcitx5 轨就是这么静默全失效的）。
    ///
    /// path 为空 → Rust 侧使用编译进二进制的**内置回退词库**。
    @discardableResult
    func load(path: String) -> Bool {
        OpiEngine.withUTF16(path) { p, n in opi_load(p, n) }
    }

    /// 繁体词库。坏路径/空 → false，且繁体模式回退简体库（不是错误）。
    @discardableResult
    func loadTrad(path: String) -> Bool {
        OpiEngine.withUTF16(path) { p, n in opi_load_trad(p, n) }
    }

    // MARK: - 状态查询（刷新 UI 用）

    /// 当前拼音缓冲 = preedit 的来源。
    func buffer() -> String { OpiEngine.takeString(opi_buffer()) }

    /// 当前模式。
    ///
    /// 兜底 `.pinyin` 只在「Rust 返回了 Swift 不认识的编码」时触发 ——
    /// 那意味着两端模式表漂移了（真发生过：缺 `traditional = 4` 时繁体被显示成拼音）。
    /// 所以**不静默**兜底：打日志，否则下次漂移还是没人发现。
    func mode() -> OpiMode {
        let raw = opi_mode()
        guard let m = OpiMode(rawValue: raw) else {
            NSLog("[OPI] 未知模式编码 %d —— 两端模式表已漂移，按拼音处理", raw)
            return .pinyin
        }
        return m
    }

    /// JSON 文本数组 → Swift 数组。Rust 侧所有多值出口都是这个形状，解析在此收口。
    ///
    /// ⚠️ 未编译：JSON 解码路径按「Rust serde_json 输出 UTF-8 → Swift String →
    /// data(using:) → JSONDecoder」写。Mac 上若发现编码问题，这里是嫌疑点。
    /// 解析失败返回空数组（不抛）—— 键盘 UI 不该因为候选栏 JSON 坏了就整体挂掉。
    private static func decodeTexts(_ json: String) -> [String] {
        guard !json.isEmpty, let data = json.data(using: .utf8) else { return [] }
        return (try? JSONDecoder().decode([String].self, from: data)) ?? []
    }

    /// **当前页**候选。候选栏显示**只用这个**。
    ///
    /// 由引擎分页（`skip(page * PAGE_SIZE).take(PAGE_SIZE)`），与 `page()` /
    /// `pageCount()` **同源，三者永远一致**。
    ///
    /// ⚠️ **不要改用 `opi_candidates(limit)` 自己按 8 切** —— 那是**从全局第 0 条起的
    /// 不分页列表**，UI 自己切片就等于把 `PAGE_SIZE` 抄了第二份：引擎改一次页大小，
    /// 这里不会报错，只会**静默错位**（高亮的页 ≠ 实际选词所在的页）。
    /// 若 Mac 上编译报 `opi_candidates_page` 未声明：那是 `macos/OpiFFI.h` 还没写上
    /// 这个出口（Rust 侧已落地，见 README「缺的出口：已落地」），**补声明，别退回自己切**。
    func candidatesPage() -> [String] {
        OpiEngine.decodeTexts(OpiEngine.takeString(opi_candidates_page()))
    }

    /// 当前候选页码（0 起）。未装载 → 0。
    ///
    /// 只用于**显示**（「第 N 页 / 共 M 页」）。**不要拿它算选词下标** ——
    /// 选词一律用 `selectPage(_:)`，页内换算归引擎。
    func page() -> Int { Int(opi_page()) }

    /// 候选总页数（**无候选 → 0**，不是 1）。给「共 N 页」用。未装载 → 0。
    func pageCount() -> Int { Int(opi_page_count()) }

    /// 前端 ⇧ 三态（0=OFF 1=SINGLE 2=LOCK）。**⇧ 高亮的唯一真源**。
    ///
    /// ⚠️ 不要把返回值缓存成「本地 shift 状态」再去 toggle：Single 会在引擎提交后
    /// **自动复位**，本地镜像必然漂移（`KeyboardLayout` 里那段注释解释的就是这件事）。
    /// 刷新时重读即可，这只是一次 C 调用。
    ///
    /// ⚠️ 未知编码（Rust 侧将来加了第四态）**不静默**兜底 —— 与 `mode()` 同一条纪律：
    /// 兜成 `.off` 会让 ⇧ 看起来「按了没反应」，有日志才查得到。
    func shiftState() -> OpiShiftState {
        let raw = opi_shift_state()
        guard let s = OpiShiftState(rawValue: raw) else {
            NSLog("[OPI] 未知 ⇧ 三态编码 %d —— 两端状态表已漂移，按 OFF 处理", raw)
            return .off
        }
        return s
    }

    /// **点候选栏第 k 个**（**页内**索引，0 起）。越界/无候选/未装载 → 空串。
    ///
    /// **这是候选栏唯一该走的选词出口。** 它和数字键选词、回车提交**同源**
    /// （`KeyRouter::select` 那一份页内换算），所以本层**不需要**知道 `PAGE_SIZE`
    /// —— 之前那句 `page * pageSize + k` 已随之删除。
    ///
    /// ⚠️ 负数是**越界**语义（返回空串），不是「从末尾数」。这里显式判负：
    /// Swift 的 `UInt32(-1)` 是**运行时陷阱**（直接崩），不像 C 那样回绕。
    /// JNI 侧对同一场景也是「负索引按越界处理」（`jni.rs` 的 `opijni_select`）。
    func selectPage(_ k: Int) -> String {
        guard k >= 0 else { return "" }
        return OpiEngine.takeString(opi_select_page(UInt32(k)))
    }

    // MARK: - 备用出口（调试 / key_event 覆盖不到的场景）
    //
    // 正常键盘路径**不要**用这些 —— 用了就等于在 Swift 侧重做路由。

    /// 选词（**全局**索引，从 0 起与完整候选列表同序）。**候选栏不要用这个** ——
    /// 用 `selectPage(_:)`；本出口留着是因为 JNI / 既有调用方按全局索引在用它。
    /// 越界返回空串。
    ///
    /// 索引空间已对着源码核过：`opi_select` → `api::select` → `router.engine_mut().select()`
    /// → **`Engine::select`（收全局下标）**，不经 `Router::select` 的页内换算。
    /// 两者**不是**一回事：`Router::select` 收页内索引（`router.rs:220`，现已 `pub`）。
    /// ⚠️ 传全局下标却按页内理解（或反之）**不会报错**，只会**选错候选**。
    func select(index: Int) -> String { OpiEngine.takeString(opi_select(index)) }

    /// 切模式。入参即 `OpiMode.rawValue`，与 Rust `mode_from_int` 的 0..=4 一致
    /// （0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional；越界 Rust 侧忽略）。
    ///
    /// ⚠️ 顺带记一处**上游注释漂移**：`cabi.rs` 里 `opi_switch_mode` 的文档注释
    /// 写的是「0=Pinyin 1=English 2=Number 3=Symbol，越界忽略」——**漏了 4**。
    /// 实际行为以 `api/mod.rs` 的 `mode_from_int` 为准（收 4）。这是 Rust 侧文档
    /// 的笔误，不影响本层（本层只转发 rawValue），但要提醒接手人别照那句注释写。
    ///
    /// ⚠️ **越界 = 静默不动作**：`cabi.rs` 走 `mode_from_int(mode)`，返 `None` 就
    /// 什么都不做，而且**函数返回 `void`，调用方查不到失败**（症状：切了模式但界面
    /// 毫无变化，且无任何日志）。
    /// 本层签名收的是 `OpiMode` 枚举、被 `rawValue` 封死在 0..=4，**当前安全**；
    /// 但将来若从菜单/配置/持久化读进**裸整数**（`Int32` / `UserDefaults` / JSON），
    /// **必须先自校验再传**，别直接 `OpiMode(rawValue: n)!` 或强转 —— 越界不报错，
    /// 只会静默失效。macOS 侧已同样注明。
    func switchMode(_ m: OpiMode) { opi_switch_mode(m.rawValue) }
    func clear() { opi_clear() }

    // MARK: - 全角 / 符号开关（三个出口 2026-09-27 落地）

    /// 全角 ⇄ 半角，返回**切换后的新状态**。
    ///
    /// 故意**不加** `@discardableResult`：返回值就是这条出口存在的理由（上游本来可以
    /// 返 void），丢掉它等于退回「按钮状态靠猜」。
    func toggleFullwidth() -> Bool { opi_toggle_fullwidth() }

    /// 全角开关的**读侧**（与 `shiftState()` / `mode()` 同构）。
    ///
    /// ⚠️ 三条硬约定**写在声明头里**（`macos/OpiFFI.h` 的「全角 / 符号开关」一节），
    /// 这里**不重抄**：这个仓库反复被「同一语义抄多份」坑过，而 Rust 侧与头文件
    /// 才是真源。要改行为先改那边。
    /// 只记两条 Swift 侧才有的：
    ///   * 它给**状态显示**用，**别**拿去推「这个键在全角下该出什么」（映射不是纯函数）。
    ///   * 本目录**还没有全角指示灯**（同 `shiftState()`：包装有了、UI 没读），
    ///     所以暂时没有陈旧值可清 —— 加指示灯时，重读写在这里。
    func fullwidthState() -> Bool { opi_fullwidth_state() }

    /// 符号面板开关：**Pinyin ⇄ Symbol 来回切**（不是「切到 Symbol」，也不是模式三态
    /// 循环 —— 那个是 `KeyboardViewController.cycleMode()`）。
    /// 返回**待上屏文本**（空串 = 无提交）。约定与出处见声明头，此处不重抄。
    func toggleSymbol() -> String { OpiEngine.takeString(opi_toggle_symbol()) }

    // MARK: - 符号库（面板数据）
    //
    // 三个出口都是**只读查询**：不碰缓冲、不改页码、不参与键路由，任意时刻可调。
    // ⚠️ 符号键**不要**送回 `key_event`：引擎的标点表会把 `,` 之类改写成中文标点/全角
    // （那是**文本**模式的行为），而用户在符号面板上点的 `,` 就是要 `,`。
    // Android 的 `SymbolPanel` 同样绕过引擎直提（`router::commitText`），
    // 本层对应 `KeyboardViewController.insert(_:)`。

    /// 常用符号块（**只有 common 块**，见 `OpiSymbolBlock` 的注释）。
    ///
    /// ⚠️ **不要缓存成「算过一次就永久用」**：Android 的 `SymbolCatalog` 在这里
    /// 踩过坑 —— 首次访问若引擎还没就绪就得到空列表，缓存空结果会让面板**从此永远空**，
    /// 要重启进程才恢复。本层按「空 = 没拿到，不是真的没有」处理：不缓存，每次问引擎。
    /// （代价是每次开面板一次 C 调用 + 一次 JSON 解码 —— 面板是低频操作，不值一个缓存。）
    func symbolBlocks() -> [OpiSymbolBlock] {
        let raw = OpiEngine.takeString(opi_symbol_blocks())
        guard let data = raw.data(using: .utf8),
              let list = try? JSONDecoder().decode([OpiSymbolBlock].self, from: data)
        else {
            // Rust 侧恒返回合法 JSON；走到这里说明两端契约漂了，别说成「没有符号」。
            if !raw.isEmpty { NSLog("[OPI] symbol_blocks JSON 解码失败：%@", raw) }
            return []
        }
        return list
    }

    /// 某个块里的符号（JSON 文本数组）。传 `OpiSymbolBlock.abiId`；nil/负值 → 空数组。
    /// ⚠️ 负 id 在 Rust 侧按**越界**处理（空数组），不会钳成块 0。
    func symbolsInBlock(id: Int16) -> [String] {
        guard id >= 0 else { return [] }
        return OpiEngine.decodeTexts(OpiEngine.takeString(opi_symbols_in_block(id)))
    }

    /// 关键字搜索符号（JSON 文本数组）。
    /// ⚠️ **空关键字返回全部条目**（`symbols::search("")`），不是「什么都没搜到」——
    /// Android 的 `SymbolCatalog.all` 正是靠这个语义取「全部」。
    func searchSymbols(keyword: String) -> [String] {
        OpiEngine.withUTF16(keyword) { p, n in
            OpiEngine.decodeTexts(OpiEngine.takeString(opi_search_symbols(p, n)))
        }
    }

    /// 常用符号（各 common 块的 `symbolsInBlock` 并集，按块序、按文本去重）。
    ///
    /// 这一份**合成**是本层唯一的「业务」——对齐 Android `SymbolCatalog.common`：
    /// 引擎只给块与块内符号，不给「常用面板该摆哪些」。空结果同样不缓存（见上）。
    func commonSymbols() -> [String] {
        var seen = Set<String>()
        var out: [String] = []
        for block in symbolBlocks() {
            guard let id = block.abiId else { continue }
            for text in symbolsInBlock(id: id) where seen.insert(text).inserted {
                out.append(text)
            }
        }
        return out
    }

    // MARK: - 用户词（学习）

    var learnerEnabled: Bool {
        get { opi_learner_enabled() }
        set { opi_set_learner(newValue) }
    }

    func exportUserWords() -> String { OpiEngine.takeString(opi_export_user_words()) }

    /// 导入用户词（JSON）。返回导入条数；**负数 = 失败**
    /// （非法 JSON / 版本不符 / 词表过大 / 引擎未装载 / 空入参）。失败不改动既有用户词。
    @discardableResult
    func importUserWords(json: String) -> Int32 {
        // 空串传下去会走 null 分支返回 -1 —— 与 Rust 注释「空串不是合法 JSON，
        // 无需特判成空词表」一致，这里不再特判。
        OpiEngine.withUTF16(json) { p, n in opi_import_user_words(p, n) }
    }

    func removeUserWord(_ text: String) {
        OpiEngine.withUTF16(text) { p, n in opi_remove_user_word(p, n) }
    }

    func clearUserWords() { opi_clear_user_words() }
}
