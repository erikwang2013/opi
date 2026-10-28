// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiSettings.swift —— 属性设置的**引擎读写面 + 落盘**。
//
// 语义真源：`crates/engine-core/src/engine_user_words.rs` 的四个 setter、
// `crates/opi-ffi/src/cabi_switches.rs` / `cabi_scheme.rs` / `cabi_dict.rs`。
// 本文件只做 Swift 值 ↔ C ABI 的转发，**不重抄任何默认值/映射**：
//   * 开关缺省值、越界钳制、未装载哨兵全部在 Rust 侧；
//   * 模糊音组名列表由 `opi_fuzzy_pair_names()` 现取（别在 Swift 里抄名表）；
//   * 方案整数 0=全拼 4=双拼（`Scheme::code`），本端设置页只出这两项，
//     与 Linux 桌面设置窗一致（9/14 键/笔画是移动端方案）。
//
// 落盘用 UserDefaults（macOS 的本地配置面），键名**逐字沿用 Linux 的配置键**
// （`opi_config.h` 的 Learn / ChinesePunct / Fuzzy / FuzzyPairs / Jianpin /
// Correction / PinyinMethod / ShuangpinLayout）—— 名字一致只为人读对照，
// 两边文件格式不通用。启动时的套用入口是 `applyPersisted()`（main.swift 调）。

import Foundation

// ---------- 导入词库（`opi_dict_list` 的 JSON 元素） ----------

/// 键序是 serde_json 的字典序（count/enabled/name/system），按**键**解码即可。
struct OpiDictEntry: Decodable {
    let name: String
    let count: Int
    let enabled: Bool
    /// 系统词典（luna/trad/stroke）不接受启停 —— UI 要隐藏开关。
    let system: Bool
}

// ---------- 设置页的枚举（纯展示值） ----------

/// 拼音方式。rawValue = 设置页 popup 的下标；`schemeCode` = C ABI 的整数。
enum OpiPinyinMethod: Int, CaseIterable {
    case full = 0
    case shuangpin = 1

    var title: String { self == .full ? "全拼" : "双拼" }
    var schemeCode: Int32 { self == .full ? 0 : 4 }

    static func from(scheme: Int32) -> OpiPinyinMethod {
        scheme == OpiPinyinMethod.shuangpin.schemeCode ? .shuangpin : .full
    }
}

/// 六种输入方案。rawValue = C ABI `opi_set_scheme` 的整数（`scheme.rs::Scheme::code`）。
/// ⚠️ 顺序**不是**声明序：1/2/3 是移动端 9/14/笔画，4 才是双拼，5 是五笔。
enum OpiScheme: Int32, CaseIterable {
    case full = 0
    case nine = 1
    case fourteen = 2
    case stroke = 3
    case shuangpin = 4
    case wubi = 5

    var title: String {
        switch self {
        case .full: return "全拼"
        case .nine: return "拼音9键"
        case .fourteen: return "拼音14键"
        case .stroke: return "笔画"
        case .shuangpin: return "双拼"
        case .wubi: return "五笔"
        }
    }

    /// 拼音方式持久化只覆盖全拼/双拼；其余走 `SchemeCode`。
    var isPinyin: Bool { self == .full || self == .shuangpin }

    static func from(code: Int32) -> OpiScheme { OpiScheme(rawValue: code) ?? .full }
}

/// 双拼布局的**展示名**（下标 = `opi_set_shuangpin_layout` 的入参）。
///
/// ⚠️ 这是本文件唯一一份名表镜像：C ABI 只有下标读写口，没有名字出口
/// （`cabi_scheme.rs` 的注释把表序钉在 `engine-core/src/shuangpin_tables.rs`）。
/// Linux 桌面同样只镜像名字（`Settings.kt` 的 `SHUANGPIN_LAYOUT_NAMES`），
/// 本端照它抄**顺序与字面**。Rust 侧加/换布局时这里要同改 —— 没有门禁，
/// 认不得的名字在引擎侧按钳制/下标处理，不会报错。
enum OpiShuangpinLayout {
    static let names = ["自然码", "微软", "小鹤", "智能ABC", "紫光", "拼音加加"]
}

// ---------- 引擎读写面 ----------

extension OpiEngine {

    // 学习 / 中文标点（Linux「常用」页的两个开关）。
    func learnerEnabled() -> Bool { opi_learner_enabled() }
    func setLearner(_ on: Bool) { opi_set_learner(on) }

    func chinesePunctEnabled() -> Bool { opi_chinese_punct() }
    func setChinesePunct(_ on: Bool) { opi_set_chinese_punct(on) }
    /// 触发键入口；返回切换后的新状态（菜单勾选可立即重画）。
    @discardableResult
    func toggleChinesePunct() -> Bool { opi_toggle_chinese_punct() }

    // 模糊音：布尔总开关 + 逐对组名。两者写同一个引擎位集，名字列表现取。
    func fuzzyEnabled() -> Bool { opi_fuzzy_enabled() }
    func setFuzzy(_ on: Bool) { opi_set_fuzzy(on) }
    /// 逐对位集（bit 序见 `OpiFFI.h`；0xFF = 全开）。设置页据此画勾选。
    func fuzzyPairsMask() -> Int32 { opi_fuzzy_pairs() }

    func fuzzyPairNames() -> [String] {
        take(opi_fuzzy_pair_names())
            .components(separatedBy: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }

    /// 逐对启用集（组名，顺序无关；引擎侧解析只留认得的名字）。
    func setFuzzyPairs(_ names: [String]) {
        withUTF16(names.joined(separator: ",")) { opi_set_fuzzy_pairs_names($0, $1) }
    }

    func jianpinEnabled() -> Bool { opi_jianpin_enabled() }
    func setJianpin(_ on: Bool) { opi_set_jianpin(on) }

    func correctionEnabled() -> Bool { opi_correction_enabled() }
    func setCorrection(_ on: Bool) { opi_set_correction(on) }

    // 输入方案 / 双拼布局。写侧**不清缓冲**（双拼布局）或**清缓冲**（方案）——
    // 差异在 Rust 侧，本层不补动作；设置窗改完由下一次按键自然刷新。
    func schemeCode() -> Int32 { opi_scheme() }
    func setScheme(code: Int32) { opi_set_scheme(code) }
    func currentScheme() -> OpiScheme { OpiScheme.from(code: schemeCode()) }

    /// 语言切换（`opi_switch_language`，2026-10-28）：换模式+方案但**保留缓冲**，
    /// 已确认片段（`opi_pending_text`）也原样留着。只用于中↔繁这类**成对**切换
    /// （iOS `toggleTraditional` 同一条）；面板换布局仍走清缓冲的 `setScheme`
    /// （切换前先 `commitPendingBeforeModeSwitch()`）。
    /// ⚠️ 入参由枚举封死；Rust 侧任一越界 = 整体忽略（不落半程切换）。
    func switchLanguage(_ mode: OpiMode, _ scheme: OpiScheme) {
        opi_switch_language(mode.rawValue, scheme.rawValue)
    }

    func shuangpinLayout() -> Int { Int(opi_shuangpin_layout()) }
    func setShuangpinLayout(_ index: Int) { opi_set_shuangpin_layout(Int32(clamping: index)) }

    // 五笔旋钮（0=纯五笔/直接字母/清空，1=混输/z键/上屏原文；个数钳 1..9）。
    func wubiStyle() -> Int { Int(opi_wubi_style()) }
    func setWubiStyle(_ v: Int) { opi_set_wubi_style(Int32(clamping: v)) }
    func wubiMixedEntry() -> Int { Int(opi_wubi_mixed_entry()) }
    func setWubiMixedEntry(_ v: Int) { opi_set_wubi_mixed_entry(Int32(clamping: v)) }
    func wubiCandidateLimit() -> Int { Int(opi_wubi_candidate_limit()) }
    func setWubiCandidateLimit(_ v: Int) { opi_set_wubi_candidate_limit(Int32(clamping: v)) }
    func wubiReturnStyle() -> Int { Int(opi_wubi_return_style()) }
    func setWubiReturnStyle(_ v: Int) { opi_set_wubi_return_style(Int32(clamping: v)) }
    func wubiAutoCommit() -> Bool { opi_wubi_auto_commit() }
    func setWubiAutoCommit(_ on: Bool) { opi_set_wubi_auto_commit(on) }
    /// 编码提示（缓冲变化时取一次；内部是全量重算，别每帧调）。
    func wubiNextKeys() -> String { take(opi_wubi_next_keys()) }

    // 用户词：JSON 导出/导入原样交给引擎，本层不解析格式。
    func clearUserWords() { opi_clear_user_words() }
    func removeUserWord(_ text: String) {
        withUTF16(text) { opi_remove_user_word($0, $1) }
    }
    /// 返回导入条数；-1 = 非法 JSON / 版本不符 / 词表过大 / 未装载。
    func importUserWords(json: String) -> Int32 {
        withUTF16(json) { opi_import_user_words($0, $1) }
    }
    func exportUserWords() -> String { take(opi_export_user_words()) }

    // ---------- 用户词自动落盘（2026-10-26） ----------
    //
    // 路径：~/Library/Application Support/opi/opi_user_words.json —— 与 fcitx5
    // $XDG_DATA_HOME/opi/opi_user_words.json、Android UserWordStore 同一个文件名。
    // 引擎（Rust）不做 IO；本层只做「引擎 JSON ↔ 文件」。唯一真源是引擎内存：
    // 导出直接问引擎，不从文件反读，故无第二份缓存可漂。
    // 失败一律 NSLog，不静默：0 字节/坏 JSON 在启动时保留原文件（不删不覆盖），
    // 下一次成功落盘才用原子写整体替换。

    static var userWordsFileURL: URL {
        dataDirectory.appendingPathComponent("opi_user_words.json")
    }

    /// 防抖状态只在 userWordsQueue 上读写：键/选词在主线程标脏，写盘在后台队列。
    private static let userWordsQueue = DispatchQueue(label: "xyz.erik.opi.user-words")
    private static var userWordsSaveWork: DispatchWorkItem?
    private static let userWordsSaveDelay: TimeInterval = 0.8

    /// 启动时恢复。缺失 = 冷启动；读不出/0 字节/坏 JSON → 日志 + **保留原文件**。
    func loadPersistedUserWords() {
        let url = OpiEngine.userWordsFileURL
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        do {
            let data = try Data(contentsOf: url)
            guard !data.isEmpty else {
                NSLog("opi: 用户词文件是 0 字节（可能是掉电残留），保留原文件：%@", url.path)
                return
            }
            guard let json = String(data: data, encoding: .utf8) else {
                NSLog("opi: 用户词文件不是 UTF-8，保留原文件：%@", url.path)
                return
            }
            let count = importUserWords(json: json)
            guard count >= 0 else {
                NSLog("opi: 用户词文件内容损坏或版本不符，保留原文件：%@", url.path)
                return
            }
            NSLog("opi: 已载入 %d 条用户词：%@", count, url.path)
        } catch {
            NSLog("opi: 用户词文件读不出（%@），保留原文件", error.localizedDescription)
        }
    }

    /// 一次成功选词后调用（主线程）。尾随防抖：连续选词只在最后一次之后 0.8s 写一次。
    func markUserWordsDirty() {
        OpiEngine.userWordsQueue.async {
            OpiEngine.userWordsSaveWork?.cancel()
            let work = DispatchWorkItem { self.saveUserWordsNow() }
            OpiEngine.userWordsSaveWork = work
            OpiEngine.userWordsQueue.asyncAfter(
                deadline: .now() + OpiEngine.userWordsSaveDelay, execute: work)
        }
    }

    /// 仅在 userWordsQueue 上调用。
    private func saveUserWordsNow() {
        let json = exportUserWords()
        // 引擎未装载时导出是空串；写成 0 字节文件会被下次启动判成损坏。
        guard !json.isEmpty else {
            NSLog("opi: 用户词导出为空（引擎未装载？），本次不写文件")
            return
        }
        let url = OpiEngine.userWordsFileURL
        do {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data(json.utf8).write(to: url, options: .atomic)
        } catch {
            NSLog("opi: 用户词写入失败（%@）：%@", url.path, error.localizedDescription)
        }
    }

    /// 清空内存 + 文件（设置页「清空用户词」）。先取消待落盘再删，防清完被写回。
    /// 返回 false = 文件删不掉（内存已清空，错误已打日志）。
    @discardableResult
    static func clearUserWordsAndFile() -> Bool {
        userWordsQueue.sync {
            userWordsSaveWork?.cancel()
            userWordsSaveWork = nil
            OpiEngine.shared.clearUserWords()
            let url = userWordsFileURL
            guard FileManager.default.fileExists(atPath: url.path) else { return true }
            do {
                try FileManager.default.removeItem(at: url)
                return true
            } catch {
                NSLog("opi: 用户词文件删除失败（%@）：%@", url.path, error.localizedDescription)
                return false
            }
        }
    }

    // 自定义短语（2026-10-23）：JSON 原样交给引擎；负数 = 失败且旧表不变。
    var customPhrasesEnabled: Bool {
        get { opi_custom_phrases_enabled() }
        set { opi_set_custom_phrases_enabled(newValue) }
    }
    func exportCustomPhrases() -> String { take(opi_export_custom_phrases()) }
    @discardableResult
    func setCustomPhrases(json: String) -> Int32 {
        withUTF16(json) { opi_set_custom_phrases($0, $1) }
    }

    // 英文自定义短语（2026-10-24）：与中文套同 schema/语义，表与开关完全独立，
    // 只在英文键盘匹配。
    var englishCustomPhrasesEnabled: Bool {
        get { opi_english_custom_phrases_enabled() }
        set { opi_set_english_custom_phrases_enabled(newValue) }
    }
    func exportEnglishCustomPhrases() -> String { take(opi_export_english_custom_phrases()) }
    @discardableResult
    func setEnglishCustomPhrases(json: String) -> Int32 {
        withUTF16(json) { opi_set_english_custom_phrases($0, $1) }
    }

    // 导入词库（M3）。系统词典不接受启停，判据看 `OpiDictEntry.system`。
    func dictList() -> [OpiDictEntry] {
        let raw = take(opi_dict_list())
        guard let data = raw.data(using: .utf8) else { return [] }
        do {
            return try JSONDecoder().decode([OpiDictEntry].self, from: data)
        } catch {
            // Rust 侧恒返回合法 JSON：走到这里说明两端契约漂了，别说成「没有词库」。
            NSLog("opi: dict_list JSON 解码失败：%@", raw)
            return []
        }
    }

    /// 导入一个 UTF-8 TSV（词<TAB>拼音<TAB>频次）。返回条目数；失败 -1。
    func importDict(path: String) -> Int32 {
        withUTF16(path) { opi_import_dict($0, $1) }
    }

    /// 启用/停用一条**导入**词库。系统词典/名字认不得/未装载 → false。
    func setDictEnabled(_ name: String, _ on: Bool) -> Bool {
        withUTF16(name) { opi_set_dict_enabled($0, $1, on) }
    }

    func removeDict(_ name: String) -> Bool {
        withUTF16(name) { opi_remove_dict($0, $1) }
    }
}

// ---------- 落盘 ----------

/// UserDefaults 键名与 Linux 配置键逐字一致（见文件头）。
enum OpiSettingsStore {
    private static var defaults: UserDefaults { .standard }

    /// 启动时套用已保存的值。**只写用户改过的键**：没有落盘值时不调 setter，
    /// 让引擎保持自己的默认（默认值的真源在 Rust 侧）。
    static func applyPersisted() {
        let engine = OpiEngine.shared

        persistedBool("Learn") { engine.setLearner($0) }
        persistedBool("ChinesePunct") { engine.setChinesePunct($0) }
        persistedBool("Fuzzy") { engine.setFuzzy($0) }
        persistedBool("Jianpin") { engine.setJianpin($0) }
        persistedBool("Correction") { engine.setCorrection($0) }

        // 空串 = 「未显式设置逐对」⇒ 回落到上面的 Fuzzy 布尔（Linux 同一条合同）。
        if let pairs = defaults.string(forKey: "FuzzyPairs"), !pairs.isEmpty {
            engine.setFuzzyPairs(pairs.components(separatedBy: ",")
                .map { $0.trimmingCharacters(in: .whitespaces) }
                .filter { !$0.isEmpty })
        }

        // 后存的 SchemeCode 优先（9/14/笔画/五笔）；否则回全拼/双拼。
        // 启动时还没有客户端、引擎刚建立（pending 必空），所以直接 setScheme；
        // 设置页/菜单/状态栏的切换一律走 `applyScheme`（先上交 pending）。
        if let raw = defaults.object(forKey: "SchemeCode") as? Int,
           let scheme = OpiScheme(rawValue: Int32(clamping: raw)), !scheme.isPinyin {
            engine.setScheme(code: scheme.rawValue)
        } else if let code = defaults.object(forKey: "PinyinMethod") as? Int,
                  code == OpiPinyinMethod.full.rawValue || code == OpiPinyinMethod.shuangpin.rawValue {
            engine.setScheme(code: OpiPinyinMethod(rawValue: code)!.schemeCode)
        }
        if let layout = defaults.object(forKey: "ShuangpinLayout") as? Int,
           OpiShuangpinLayout.names.indices.contains(layout) {
            engine.setShuangpinLayout(layout)
        }

        if let v = defaults.object(forKey: "WubiStyle") as? Int { engine.setWubiStyle(v) }
        if let v = defaults.object(forKey: "WubiMixedEntry") as? Int { engine.setWubiMixedEntry(v) }
        if let v = defaults.object(forKey: "WubiCandidateLimit") as? Int {
            engine.setWubiCandidateLimit(v)
        }
        if let v = defaults.object(forKey: "WubiReturn") as? Int { engine.setWubiReturnStyle(v) }
        persistedBool("WubiAutoCommit") { engine.setWubiAutoCommit($0) }
    }

    /// 切方案并落盘（设置页/状态栏共用；不要在两处各写一份键名）。
    /// ⚠️ 先交出已确认未提交片段（`opi_pending_text`）：`setScheme` 不清它们，
    /// 不交会在换方案后把同一段再上屏一次（见 `commitPendingBeforeModeSwitch`）。
    static func applyScheme(_ scheme: OpiScheme) {
        OpiClientHub.shared.commitPendingBeforeModeSwitch()
        OpiEngine.shared.setScheme(code: scheme.rawValue)
        if scheme.isPinyin {
            set("PinyinMethod", scheme == .shuangpin
                ? OpiPinyinMethod.shuangpin.rawValue : OpiPinyinMethod.full.rawValue)
            remove("SchemeCode")
        } else {
            set("SchemeCode", Int(scheme.rawValue))
        }
    }

    /// 自绘贴纸页的可见性（Linux `ShowOwn`，默认关）。
    static var showOwn: Bool {
        get { bool("ShowOwn", default: false) }
        set { set("ShowOwn", newValue) }
    }

    /// 智能联想（Linux `Association`，默认关）。本端无 C ABI 出口，是纯宿主开关：
    /// 关闭时 `refresh` 在联想候选推给候选窗之前整批挡掉（同 iOS/Android）。
    static var association: Bool {
        get { bool("Association", default: false) }
        set { set("Association", newValue) }
    }

    /// 括号自动配对（macOS 本地开关，默认开；iOS/Android 无开关、恒开）。
    /// 纯宿主开关；英文模式 / 安全输入框仍然不配对（见 `BracketPairing.swift`）。
    static var bracketPairing: Bool {
        get { bool("BracketPairing", default: true) }
        set { set("BracketPairing", newValue) }
    }

    static func bool(_ key: String, default fallback: Bool) -> Bool {
        guard defaults.object(forKey: key) != nil else { return fallback }
        return defaults.bool(forKey: key)
    }

    static func int(_ key: String, default fallback: Int) -> Int {
        defaults.object(forKey: key) as? Int ?? fallback
    }

    static func set(_ key: String, _ value: Bool) { defaults.set(value, forKey: key) }
    static func set(_ key: String, _ value: Int) { defaults.set(value, forKey: key) }
    static func set(_ key: String, _ value: String) { defaults.set(value, forKey: key) }
    static func remove(_ key: String) { defaults.removeObject(forKey: key) }

    /// 停用名单（Linux 同名键 `DisabledDicts`，逗号分隔；空 = 全部启用）。
    /// ⚠️ 名字含逗号的词库表达不了 —— 设置页禁止翻转并提示。
    static func disabledDicts() -> Set<String> {
        let raw = defaults.string(forKey: "DisabledDicts") ?? ""
        return Set(raw.components(separatedBy: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty })
    }

    static func setDisabledDicts(_ names: Set<String>) {
        set("DisabledDicts", names.sorted().joined(separator: ","))
    }

    private static func persistedBool(_ key: String, _ apply: (Bool) -> Void) {
        guard defaults.object(forKey: key) != nil else { return }
        apply(defaults.bool(forKey: key))
    }
}
