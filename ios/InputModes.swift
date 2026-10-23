// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **CI 已真编译**（apple.yml 的 iOS job，2026-10-15）· **从未上机运行**
//
// 本文件在 Linux 上编写。**已过 `swiftc ios/*.swift`（-typecheck + -emit-library）** ——
// 2026-10-15 那次全目录真编译（同一轮还当场抓出一处闭包里的 `continue`，见 RarePanel.swift）；
// 但从未在设备或模拟器上跑过：**行为仍未验证**，别把「编过」读成「能用」。
// UIKit 是 Apple 独有框架，本机 `import UIKit` 就会断。
//
// **输入模式**（2026-10-14）：菜单里给用户看的那九项 = 键面 × 引擎方案 × 语言模式
// 的组合。三根轴分清楚（Android `InputMode.kt` 的头注就是这条，别混）：
//   · 方案 `OpiScheme`（引擎）：按键 → 字母类（9 键 / 14 键 / 笔画 / 全键盘）；
//   · 模式 `OpiMode`（引擎）：语言变体（拼音 / 繁体 / 英文），**与方案正交**；
//   · 本枚举（UI）：每项映射到上面两个引擎轴。
// 大 9 键与拼音 9 键是**同一引擎配置**（只差字号）；生僻字**不进引擎**（面板式）。
//
// 数值/键位/次序的**参照实现是 Android**（`android/.../ime/InputMode.kt`、
// `keyboard/NineKeyKeyboard.kt`、`FourteenKeyKeyboard.kt`、`StrokeKeyboard.kt`）——
// 抄语义与键位，不抄 Kotlin 语法。方案整数码的**唯一真源**是
// `crates/engine-core/src/scheme.rs` 的 `code()`（见 `OpiEngineScheme.swift`）。
//
// 本文件装两件东西（都是「数据 + 纯算法」，不碰视图树生命周期）：
//   1. `InputMode`：九项枚举与它们的引擎映射；
//   2. `ModeKeyTables` / `ModeKeyRenderer`：各模式的**键位表**与**尺寸算法**。
// 「选中一项后对引擎/键面做什么」在 `InputModeApply.swift`（2026-10-23 拆出：
// 那部分要加懒加载与合并重建，本文件 499 行顶到了 500 行门禁）。
// 摆位与触摸接线仍在 `KeyboardLayout.swift`（那是它的本职）。

import UIKit

/// 输入模式（菜单里的九项）。次序 = 声明次序 = 菜单次序（对齐 Android 的 `entries`）。
enum InputMode: Int, CaseIterable {
    case pinyin26
    case english26
    case pinyin9
    case pinyin14
    /// 大 9 键：与 `pinyin9` **同一引擎配置**，只差字号（Android 同款：`big = true`）。
    case big9
    case stroke
    /// 生僻字：**面板式，不进引擎** —— 选中它只切面板视图，不动 scheme/mode。
    case rare
    /// 双拼：键面就是 26 键，解码与布局在引擎侧。
    case shuangpin
    /// 五笔：键面同样是 26 键，码表另装（见 `OpiEngineWubi.swift`）。
    case wubi

    /// 引擎方案（`opi_set_scheme` 的入参）。
    ///
    /// ⚠️ `rare` 报 `.full` 是**如实**的（Android 同）：它根本不设 scheme，
    /// 只是给出「若不进面板，键面该是 26 键」这个信息。**别**拿它当「点 rare 会
    /// 把引擎切到全拼」——进面板的那条路一个字节都没碰引擎。
    var scheme: OpiScheme {
        switch self {
        case .pinyin26, .english26, .rare: return .full
        case .pinyin9, .big9: return .nine
        case .pinyin14: return .fourteen
        case .stroke: return .stroke
        case .shuangpin: return .shuangpin
        case .wubi: return .wubi
        }
    }

    /// 引擎语言模式（`opi_switch_mode` 的入参）。除英文 26 键外全是拼音
    /// （繁体不是模式、是三态循环里的中间档，不进这张表）。
    var engineMode: OpiMode {
        switch self {
        case .english26: return .english
        default: return .pinyin
        }
    }

    /// 大键面（仅字号差异）。
    var big: Bool { self == .big9 }

    /// 面板式（生僻字）：UI 层拦下，不进引擎。
    var panel: Bool { self == .rare }

    /// 该键面的模式键是否**只切简/繁**（9 键 / 大 9 键 / 14 键 / 笔画）。
    /// 26 键没有独立英文键，仍走中→繁→英三档循环（Android `ImeScreen` 同判）。
    var usesJianFanModeKey: Bool {
        switch self {
        case .pinyin9, .big9, .pinyin14, .stroke: return true
        default: return false
        }
    }

    /// 菜单里的名字（与 Android 逐字相同，别各端各叫）。
    var label: String {
        switch self {
        case .pinyin26: return "拼音26键"
        case .english26: return "英文26键"
        case .pinyin9: return "拼音9键"
        case .pinyin14: return "拼音14键"
        case .big9: return "拼音大9键"
        case .stroke: return "笔画键盘"
        case .rare: return "生僻字键盘"
        case .shuangpin: return "拼音双拼"
        case .wubi: return "五笔"
        }
    }
}

// MARK: - 键位行模型

/// 一行里的一个格子：单键，或**竖排一列**（9 键的左标点列与右功能列）。
///
/// 为什么要「列」这个形状：9 键的主区是 5 列（标点列 | 3×3 数字格拆成的三列 |
/// ⌫/重输/空格列），用扁平的「一行若干键」表达不出来 —— 硬拆成 3 行会让左列
/// 重复三次，那才是真的错。
enum ModeCell {
    case key(KeySpec)
    /// 竖排一列（列内等高）。列宽**恒为 1**：Android 的 9 键五列权重都是 1；
    /// 哪天真要变宽再加参数，别提前造旋钮。
    case column([KeySpec])

    var widthUnits: CGFloat {
        switch self {
        case .key(let spec): return spec.widthUnits
        case .column: return 1
        }
    }
}

/// 一行：格子 + **行高权重**（1 = 普通行）。
///
/// 行高权重存在的理由：9 键主区是底行的 3 倍高（Android `weight(3f)` / `weight(1f)`）。
/// 26 键的两层与 14 键/笔画全是等权，权重恒 1 —— 所以这套模型对旧布局是**恒等变换**。
struct ModeRow {
    let heightUnits: CGFloat
    let cells: [ModeCell]

    init(_ cells: [ModeCell], heightUnits: CGFloat = 1) {
        self.cells = cells
        self.heightUnits = heightUnits
    }

    /// 普通行（全是单键）的便捷构造。
    static func plain(_ specs: [KeySpec], heightUnits: CGFloat = 1) -> ModeRow {
        ModeRow(specs.map { ModeCell.key($0) }, heightUnits: heightUnits)
    }
}

// MARK: - 各模式的键位表

/// 键位表（**协议级**：发送的键值决定引擎收到什么，别随手改）。
///
/// ⚠️ 两张表的交叉核对在 Android 侧有门禁（`NineKeyLayoutTest` / `FourteenKeyLayoutTest`
/// 拿 `scheme.rs` 的类表逐格比）。本端**没有**那种门禁（没有 Swift 测试基建），
/// 所以这里的字母组是**手抄** —— Mac 上要做的第一件事之一就是对着
/// `crates/engine-core/src/scheme.rs` 的 `key_class` 表逐格核一遍。
enum ModeKeyTables {

    /// 🌐：短按在 OPI 内循环语言，**长按**才走系统输入法（2026-10-23：避免 9 键底行
    /// 的繁/123/英文被误判为系统切换；普通点击不再可能离开 OPI）。
    static let systemInputKey = KeySpec("🌐", function: .cycleMode, longPressFunction: .nextInputMode)

    /// 按当前模式取键面（字母层的行）。数字层与所有模式共用 `numberRows`。
    ///
    /// `rare`（面板式）回落到 26 键：Android 的 `layoutFor` 对「上一个模式是面板式
    /// 且引擎没变」保持画面板，收起面板后底下画的就是 26 键（`RARE.scheme == FULL`）。
    /// 本端同此 —— 收起生僻字面板回到 26 键，而 `inputMode` 仍是 `rare`
    /// （所以菜单里的高亮仍在「生僻字键盘」上，与 Android 的 chip 高亮一致）。
    static func rows(for mode: InputMode, modeLabel: String) -> [ModeRow] {
        switch mode {
        case .pinyin26, .english26, .shuangpin, .wubi, .rare:
            return letterRows(modeLabel: modeLabel, english: mode == .english26)
        // 大 9 键与拼音 9 键**同一张表**，只差字号（`big` 是唯一的分叉）。
        case .pinyin9, .big9: return nineKeyRows(modeLabel: modeLabel, big: mode.big)
        case .pinyin14: return fourteenKeyRows(modeLabel: modeLabel)
        case .stroke: return strokeRows(modeLabel: modeLabel)
        }
    }

    // MARK: 26 键（字母层 / 数字层）

    /// 字母层三行 + 功能行（**从 `KeyboardLayout.swift` 机械搬来**，只把
    /// `modeLabel` 从实例状态改成入参；行内容一字未改）。
    ///
    /// 字母键一律送**小写码点 + states 0**，不自己做大写化：keys.rs 的约定是
    /// 「大小写由码点携带」是给**物理键盘**的（平台已应用 ⇧）；而软键盘的 ⇧ 走
    /// `handle_shift` 维护的引擎内状态机，由它决定英文模式的 Input 与拼音
    /// composer 的大小写。这里若再自己转一次大写，就是第五份大小写逻辑。
    static func letterRows(modeLabel: String, english: Bool = false) -> [ModeRow] {
        let top = "qwertyuiop".map { KeySpec.char($0) }
        let mid = "asdfghjkl".map { KeySpec.char($0) }
        var bottom: [KeySpec] = [
            // ⇧：单击/长按的**语义**（Off→Single→Lock）全在 Rust 的 handle_shift，
            // 这里只把「长按」翻成 stateLongPressed 位。states 不送 KEY_STATE_SHIFT：
            // handle_shift 不读它。
            KeySpec("⇧", keyval: OpiKey.shift, widthUnits: 1.4)
        ]
        bottom += "zxcvbnm".map { KeySpec.char($0) }
        bottom.append(
            KeySpec("⌫", keyval: OpiKey.backspace, fallback: .deleteBackward, longPressRepeatsBackspace: true, widthUnits: 1.4)
        )

        // 2026-10-23 用户裁决（英文 26 键）：底行在空格前加 `,` `.`，空格变小；
        // ⌫ 本来就在第三行最右、↵ 在底行，与 Android/Harmony 互换后的布局一致。
        // 标点走 `.commitText` **直提**，不进引擎 —— 英文模式不该被中文标点表改写。
        let punctuation: [KeySpec] = english
            ? [KeySpec(",", function: .commitText(",")), KeySpec(".", function: .commitText("."))]
            : []
        let spaceUnits: CGFloat = english ? 2.5 : 5
        let funcRow: [KeySpec] = [
            systemInputKey,
            KeySpec("123", function: .showNumbers),
            // 键面字 = **点它会切到哪**（2026-10-12 晚裁决，见 KeyboardViewController 的
            // modeKeyLabel 注释），不是写死的 "中/英"。
            // 长按 = 打开输入模式面板（2026-10-14）：Android 从工具条进，iOS 没有工具条，
            // 长按模式键是**同一件事的 iOS 说法**（主流输入法在 iOS 上也是长按中/英出菜单）。
            KeySpec(modeLabel, function: .cycleMode, longPressFunction: .showModes),
        ] + punctuation + [
            KeySpec("空格", keyval: OpiKey.space, fallback: .text(" "), widthUnits: spaceUnits),
            KeySpec("回车", keyval: OpiKey.ret, fallback: .text("\n"), widthUnits: 1.6),
        ]
        return [
            ModeRow.plain(top), ModeRow.plain(mid), ModeRow.plain(bottom), ModeRow.plain(funcRow),
        ]
    }

    /// 数字/标点层（**最小集合**；从 `KeyboardLayout.swift` 机械搬来）。
    ///
    /// 这里没有「符号」键位表：符号来自引擎的符号库（`#+=` 切到 `.symbols` 层），
    /// 不再在 Swift 里另抄一份符号表 —— Android 同样只把符号交给引擎/面板。
    /// 数字键面**没有模式键**（Android `NumberPad` 同样没有）：顶部槽位有工具条，
    /// 模式切换在字母层的模式键上；少一个键也免了「数字层模式键的循环语义与
    /// 9/14/笔画只切简繁不一致」的两套标签。
    static func numberRows() -> [ModeRow] {
        let r1 = "1234567890".map { KeySpec.char($0) }
        let r2 = ["-", "/", ":", ";", "(", ")", "$", "&", "@", "\""].map { KeySpec.char(Character($0)) }
        let r3: [KeySpec] = [
            KeySpec("ABC", function: .showLetters, widthUnits: 1.4),
            KeySpec.char("."),
            KeySpec.char(","),
            KeySpec.char("?"),
            KeySpec.char("!"),
            KeySpec.char("'"),
            KeySpec("⌫", keyval: OpiKey.backspace, fallback: .deleteBackward, longPressRepeatsBackspace: true, widthUnits: 1.4),
        ]
        let funcRow: [KeySpec] = [
            systemInputKey,
            KeySpec("#+=", function: .showSymbols),
            // 表情入口（2026-10-13）：紧挨符号入口。Android 的表情是从**工具条**进的
            // （`SymbolTab.Emoji`，任何一层都能直达）；iOS 这一版先只加这一个入口 ——
            // 与上面「#+= → 符号」同级，别的地点（字母层功能行）保持原样。
            KeySpec("😊", function: .showEmoji),
            KeySpec("空格", keyval: OpiKey.space, fallback: .text(" "), widthUnits: 5),
            KeySpec("回车", keyval: OpiKey.ret, fallback: .text("\n"), widthUnits: 1.6),
        ]
        return [
            ModeRow.plain(r1), ModeRow.plain(r2), ModeRow.plain(r3), ModeRow.plain(funcRow),
        ]
    }

    // MARK: 9 键（拼音 9 键 / 大 9 键）

    /// 拼音 9 键（T9）。主区 5 列（权重 3）+ 底行（权重 1）。
    ///
    /// 左列**空闲时是标点列，输入中换成拼音读数**（2026-10-21 C ABI 新增
    /// `opi_candidate_readings` 后补齐；浮层见 `CandidateColumns.swift`）。
    /// 数字键在 9 键方案下**是输入键**（引擎 Nine 方案收进缓冲），所以不需要数字选词 UI。
    static func nineKeyRows(modeLabel: String, big: Bool) -> [ModeRow] {
        // 字号（Android：字母 21sp / 大 30sp；多标点键收一档）。
        // 数字行/字母行两级：`digitSpec` 的 `lineScales: [0.62, 1]`（2026-10-23）。
        let letterSize: CGFloat = big ? 30 : 21

        // ---- 左列：标点（主流同款一列）----
        // 发送键都取引擎中文标点表（punctuation.rs 的 CHINESE_PUNCT）**已有的键**：
        // `,`→，`.`→。`\`→、`?`→？`!`→！ —— 与 26 键的 `,` 是**同一条路**
        // （不是 UI 直提）。
        let strip = [
            punctSpec("，", ","),
            punctSpec("。", "."),
            punctSpec("、", "\\"),
            punctSpec("？", "?"),
            punctSpec("！", "!"),
        ]
        // ---- 中间三列 = Android 的 3×3 数字格按**列**取（行主序 → 列）----
        let col1 = [
            // 主区域第一格 = **分词键**（Android 2026-10-15 用户裁决「第一符号按钮
            // 改成分词按钮」）：往引擎缓冲插音节边界（`xi|an`，见 `scheme.rs`
            // `matches_segmented`）。空缓冲由控制器拦住（空缓冲的 `'` 是引号）。
            KeySpec("分词", function: .segment, fontSize: letterSize * 0.72),
            digitSpec("4", "ghi", letterSize),
            digitSpec("7", "pqrs", letterSize),
        ]
        let col2 = [
            digitSpec("2", "abc", letterSize),
            digitSpec("5", "jkl", letterSize),
            digitSpec("8", "tuv", letterSize),
        ]
        let col3 = [
            digitSpec("3", "def", letterSize),
            digitSpec("6", "mno", letterSize),
            digitSpec("9", "wxyz", letterSize),
        ]
        // ---- 右列：⌫ / 重输 / 空格（2026-10-10 Android 用户裁决：从底行搬进右列）----
        let right = [
            KeySpec("⌫", keyval: OpiKey.backspace, fallback: .deleteBackward, longPressRepeatsBackspace: true),
            // 重输 = 清掉当前组合重来（缓冲空时是空操作，engine.clear() 幂等）。
            // 分词键在主区第一格（见 col1），不在这一列。
            KeySpec("重输", function: .retype),
            KeySpec("空格", keyval: OpiKey.space, fallback: .text(" ")),
        ]
        let main = ModeRow(
            [
                .column(strip), .column(col1), .column(col2), .column(col3), .column(right),
            ],
            heightUnits: 3
        )
        // ---- 底行：[🌐][符][中/繁][123][英文][↵] ----
        let funcRow = ModeRow.plain([
            systemInputKey,
            KeySpec("符", function: .showSymbols),
            KeySpec(modeLabel, function: .toggleTraditional, longPressFunction: .showModes),
            KeySpec("123", function: .showNumbers),
            // 「英文」= 切到英文 26 键（与菜单里的「英文26键」**同一条路**：
            // 走 `.selectInputMode`，不是本地换层 —— 语义含「换方案 + 换模式」）。
            KeySpec("英文", function: .selectInputMode(.english26)),
            KeySpec("↵", keyval: OpiKey.ret, fallback: .text("\n")),
        ])
        return [main, funcRow]
    }

    // MARK: 14 键

    /// 拼音 14 键：两字母一键，**发送字母对的第一个字母**（`qw→q`）。
    ///
    /// ⚠️ **协议级约束**：发送键必须等于 `scheme.rs` 的 `Scheme::Fourteen.key_class`
    /// 规范符号，键面的两个字母必须**恰好**是那一对（`l`/`m` 是单字母键）。
    /// 改这张表 = 改协议，四端一起改（Android 有 `FourteenKeyLayoutTest` 钉着）。
    static func fourteenKeyRows(modeLabel: String) -> [ModeRow] {
        let r1 = [pairSpec("qw"), pairSpec("er"), pairSpec("ty"), pairSpec("ui"), pairSpec("op")]
        let r2 = [pairSpec("as"), pairSpec("df"), pairSpec("gh"), pairSpec("jk"), pairSpec("l")]
        let r3 = [pairSpec("zx"), pairSpec("cv"), pairSpec("bn"), pairSpec("m")]
        // 底行：[🌐][符][中/繁][英文][空格(2)][⌫][↵]（Android 同序；重输是 9 键专属）。
        // 「英文」是**独立入口**：模式键只切简/繁后，不补这枚英文在本盘就不可达。
        let funcRow: [KeySpec] = [
            systemInputKey,
            KeySpec("符", function: .showSymbols),
            KeySpec(modeLabel, function: .toggleTraditional, longPressFunction: .showModes),
            KeySpec("英文", function: .selectInputMode(.english26)),
            KeySpec("空格", keyval: OpiKey.space, fallback: .text(" "), widthUnits: 2),
            KeySpec("⌫", keyval: OpiKey.backspace, fallback: .deleteBackward, longPressRepeatsBackspace: true),
            KeySpec("↵", keyval: OpiKey.ret, fallback: .text("\n")),
        ]
        return [ModeRow.plain(r1), ModeRow.plain(r2), ModeRow.plain(r3), ModeRow.plain(funcRow)]
    }

    // MARK: 笔画

    /// 笔画键盘（五笔画）。发送 `h/s/p/n/z`（横竖撇捺折）—— **普通字母**，
    /// 与 `scheme.rs` 的 `Scheme::Stroke` 约定一致（那一路换的是**词典**：
    /// `stroke.opid` 的键就是笔画串，词是单字；匹配走普通前缀）。
    ///
    /// ⚠️ 笔画串的顺序 = 书写顺序（首笔在前）；词典没装时候选为空（可选增强语义），
    /// 键盘照常可打。
    static func strokeRows(modeLabel: String) -> [ModeRow] {
        let keys = [
            strokeSpec("一", "h", "横"),
            strokeSpec("丨", "s", "竖"),
            strokeSpec("丿", "p", "撇"),
            strokeSpec("丶", "n", "捺"),
            strokeSpec("乙", "z", "折"),
        ]
        // 功能行含「生僻字」入口（找不到字时下一步就是它）：🌐 / 符 / 模式 / 英文 /
        // 生僻字(1.4) / 空格(2) / ⌫ / ↵（Android 同序；英文是模式键只切简繁后的
        // 独立入口）。
        let funcRow: [KeySpec] = [
            systemInputKey,
            KeySpec("符", function: .showSymbols),
            KeySpec(modeLabel, function: .toggleTraditional, longPressFunction: .showModes),
            KeySpec("英文", function: .selectInputMode(.english26)),
            KeySpec("生僻字", function: .showRare, widthUnits: 1.4),
            KeySpec("空格", keyval: OpiKey.space, fallback: .text(" "), widthUnits: 2),
            KeySpec("⌫", keyval: OpiKey.backspace, fallback: .deleteBackward, longPressRepeatsBackspace: true),
            KeySpec("↵", keyval: OpiKey.ret, fallback: .text("\n")),
        ]
        return [ModeRow.plain(keys), ModeRow.plain(funcRow)]
    }

    // MARK: - 小构造器

    /// 标点键：**键面**写中文标点，**发送**引擎标点表已有的 ASCII 键。
    ///
    /// ⚠️ **不能**用 `KeySpec.char(label)`：它取键面**首个标量**的码点（「，」= U+FF0C），
    /// 发下去的是另一个键 —— 而引擎的标点表收的入键是 ASCII 的 `,`。
    /// 兜底文本取**发送键**（不是键面）：英文模式下引擎不管标点，插进去的就是 `,`
    /// （Android 同：onKey 送 key，直通时插 key）。
    private static func punctSpec(_ label: String, _ send: String, fontSize: CGFloat = 20) -> KeySpec {
        let kv = send.unicodeScalars.first.map { OpiKey.printable($0) } ?? 0
        return KeySpec(label, keyval: kv, fallback: .text(send), fontSize: fontSize)
    }

    /// 9 键数字键：键面「2 / abc」两行（数字在上、字母组在下 —— Android 2026-10-09
    /// 用户裁决「字母为主、数字缩小到下面」，2026-10-23 起用 `lineScales` 落成
    /// 数字 0.62 / 字母 1 的两级字号），发送数字本身。
    private static func digitSpec(_ digit: String, _ hint: String, _ size: CGFloat) -> KeySpec {
        let kv = digit.unicodeScalars.first.map { OpiKey.printable($0) } ?? 0
        return KeySpec("\(digit)\n\(hint)", keyval: kv, fallback: .text(digit),
                       fontSize: size, lineScales: [0.62, 1])
    }

    /// 14 键两字母键：键面两个字母，发送**首个**字母。
    private static func pairSpec(_ pair: String) -> KeySpec {
        let kv = pair.unicodeScalars.first.map { OpiKey.printable($0) } ?? 0
        return KeySpec(pair, keyval: kv, fallback: .text(pair), fontSize: 22)
    }

    /// 笔画键：键面「一 / 横」，发送 `h` 等字母。字形大、名称小（同 Android 两级）。
    private static func strokeSpec(_ glyph: String, _ send: String, _ name: String) -> KeySpec {
        let kv = send.unicodeScalars.first.map { OpiKey.printable($0) } ?? 0
        return KeySpec("\(glyph)\n\(name)", keyval: kv, fallback: .text(send),
                       fontSize: 26, lineScales: [1, 0.62])
    }
}

// MARK: - 键面渲染

// 尺寸算法在 `ModeKeyRenderer.swift`（`[ModeRow]` → 视图树；单开一个文件的理由是
// 那边压 500 行上限，见它的头注）。本文件只管**数据**：模式定义与各模式的键位表。
