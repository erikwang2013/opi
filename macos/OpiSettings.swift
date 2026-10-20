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

    func shuangpinLayout() -> Int { Int(opi_shuangpin_layout()) }
    func setShuangpinLayout(_ index: Int) { opi_set_shuangpin_layout(Int32(clamping: index)) }

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

        if let code = defaults.object(forKey: "PinyinMethod") as? Int,
           code == OpiPinyinMethod.full.rawValue || code == OpiPinyinMethod.shuangpin.rawValue {
            engine.setScheme(code: OpiPinyinMethod(rawValue: code)!.schemeCode)
        }
        if let layout = defaults.object(forKey: "ShuangpinLayout") as? Int,
           OpiShuangpinLayout.names.indices.contains(layout) {
            engine.setShuangpinLayout(layout)
        }
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
