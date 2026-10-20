// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiMenu.swift —— 输入法菜单（macOS 的「状态栏」等价物）+ 跨窗口提交收口。
//
// Linux 的状态栏按钮顺序（desktop/…/SkinAndStatus.kt 的 StatusBarButton）是
// LOGO/LANG/PUNCT/TRAD/ENPRO/MODE/FULLWIDTH/VK/SYMBOLS/SETTINGS。InputMethodKit
// **没有常驻状态栏**：输入法只在「输入法菜单」里出现，所以本文件把其中可实现的
// 项落成菜单项，顺序按 Linux：
//   LANG → 英文模式   TRAD → 繁体模式   PUNCT → 中文标点
//   MODE → 输入方案（全拼/双拼）          FULLWIDTH → 全角
//   SYMBOLS → 符号面板…                    SETTINGS → 设置…
// 未接：ENPRO（fcitx5 专有的 englishpro 控制面，C ABI 无对应出口）、VK（系统键盘
// 查看器由用户自开）、LOGO（Linux 上是皮肤/关于入口，这里并进「关于 OPI」）。
//
// ⚠️ 菜单项 dispatch 走 IMK 的 `doCommand(by:command:)`（info 字典的
// `kIMKCommandMenuItemName` 带回 NSMenuItem，按 tag 分派）——这是 IMK 文档的
// 标准路，但**没有在真机验证**。同时实现直连 action 兜底：两条路任一生效即可。

import AppKit
import InputMethodKit

// ---------- 跨窗口提交 ----------

/// 最近一次获得焦点的输入客户端。
///
/// 为什么需要：IMK 的 `client()` 只在控制器活着/激活时可靠；符号面板与设置窗是
/// 独立 NSPanel，从它们的按钮上屏时没有 `handle(_:client:)` 的 sender。这里在
/// 每次按键/激活时记一份**弱引用**（不强留宿主应用），面板上屏时优先用它。
/// 客户端已释放（应用退出）→ 调用方回落到 `client()` / 无操作。
final class OpiClientHub {
    static let shared = OpiClientHub()

    private(set) weak var client: AnyObject?
    private weak var controller: OpiInputController?

    private init() {}

    func focus(_ controller: OpiInputController, client: AnyObject?) {
        self.controller = controller
        if let client { self.client = client }
    }

    func commit(_ text: String) {
        controller?.commitFromOutside(text)
    }
}

// ---------- 菜单 ----------

enum OpiMenuTag: Int {
    case toggleEnglish = 1
    case toggleTraditional
    case toggleChinesePunct
    case toggleFullwidth
    case schemeFull
    case schemeShuangpin
    case symbolPanel
    case settings
    case about
}

enum OpiMenu {
    /// 每次打开菜单都现建：勾选状态直接读引擎（不做缓存，缓存会显示旧模式）。
    static func build(engine: OpiEngine, target: OpiInputController) -> NSMenu {
        let menu = NSMenu(title: "OPI")
        menu.autoenablesItems = false

        menu.addItem(item("英文模式", target: target, tag: .toggleEnglish,
                          on: engine.mode() == .english))
        menu.addItem(item("繁体模式", target: target, tag: .toggleTraditional,
                          on: engine.mode() == .traditional))
        menu.addItem(.separator())
        menu.addItem(item("中文标点", target: target, tag: .toggleChinesePunct,
                          on: engine.chinesePunctEnabled()))
        menu.addItem(item("全角字符", target: target, tag: .toggleFullwidth,
                          on: engine.fullwidthState()))

        // MODE：Linux 桌面的方案芯片只出全拼/双拼（9/14 键/笔画是移动端方案）。
        let scheme = NSMenuItem(title: "输入方案", action: nil, keyEquivalent: "")
        let sub = NSMenu(title: "输入方案")
        sub.autoenablesItems = false
        sub.addItem(item("全拼", target: target, tag: .schemeFull,
                         on: engine.schemeCode() == OpiPinyinMethod.full.schemeCode))
        sub.addItem(item("双拼", target: target, tag: .schemeShuangpin,
                         on: engine.schemeCode() == OpiPinyinMethod.shuangpin.schemeCode))
        scheme.submenu = sub
        menu.addItem(scheme)

        menu.addItem(.separator())
        // 快捷键只写在标题里，不设 keyEquivalent：IMK 的 handle(_:client:) 已经
        // 截获这些组合，菜单再注册一份会造成「菜单开着时按一次执行两次」。
        menu.addItem(item("符号面板…  ⌃⇧Z", target: target, tag: .symbolPanel))
        menu.addItem(item("设置…", target: target, tag: .settings))
        menu.addItem(.separator())
        menu.addItem(item("关于 OPI", target: target, tag: .about))
        return menu
    }

    /// 分派菜单动作。`controller` 用来在模式变化后刷新 preedit/候选。
    static func perform(tag: Int, controller: OpiInputController) {
        guard let tag = OpiMenuTag(rawValue: tag) else { return }
        let engine = OpiEngine.shared
        switch tag {
        case .toggleEnglish:
            // 与 Ctrl+' 同判（InputController.hotkey 的 toggleEnglish 支）。
            engine.switchMode(engine.mode() == .english ? .pinyin : .english)
            controller.refreshFromOutside()
        case .toggleTraditional:
            // 与 `opi_mode.h` 的 opiToggleTraditional 同判：在繁体就回拼音，否则进繁体。
            engine.switchMode(engine.mode() == .traditional ? .pinyin : .traditional)
            controller.refreshFromOutside()
        case .toggleChinesePunct:
            // 只翻一个偏好位，不动缓冲/候选 —— 不需要刷新。
            engine.toggleChinesePunct()
        case .toggleFullwidth:
            engine.toggleFullwidth()
        case .schemeFull:
            engine.setScheme(code: OpiPinyinMethod.full.schemeCode)
            OpiSettingsStore.set("PinyinMethod", OpiPinyinMethod.full.rawValue)
            controller.refreshFromOutside()
        case .schemeShuangpin:
            engine.setScheme(code: OpiPinyinMethod.shuangpin.schemeCode)
            OpiSettingsStore.set("PinyinMethod", OpiPinyinMethod.shuangpin.rawValue)
            controller.refreshFromOutside()
        case .symbolPanel:
            OpiSymbolPanel.shared.show()
        case .settings:
            OpiSettingsWindow.shared.show()
        case .about:
            OpiSettingsWindow.shared.show(page: .about)
        }
    }

    private static func item(_ title: String, target: OpiInputController,
                             tag: OpiMenuTag, on: Bool = false) -> NSMenuItem {
        // target 明确指向控制器：action 直接落到 menuCommand(_:)，不赌 IMK 的
        // responder 链。doCommand 的覆写仍保留（部分 IMK 版本会把菜单动作转成它）。
        let it = NSMenuItem(title: title,
                            action: #selector(OpiInputController.menuCommand(_:)),
                            keyEquivalent: "")
        it.target = target
        it.tag = tag.rawValue
        it.state = on ? .on : .off
        return it
    }
}
