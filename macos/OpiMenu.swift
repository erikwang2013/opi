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

    /// 换模式/方案前交出**已确认未提交**的逐字片段（`opi_pending_text`，同 iOS
    /// 的 `commitPendingBeforeModeSwitch`）：`opi_switch_mode` 会静默丢掉它们。
    /// 先 clear 再插入 —— `opi_set_scheme` **不清**这些片段，不 clear 换了方案后
    /// 同一段会再上屏一次。未确认缓冲（9 键里是数字）绝不直提，同 iOS。
    /// 只给「清缓冲」的那条路（`switchMode` / `setScheme`）用；中↔繁这类**成对**
    /// 切换走 `engine.switchLanguage`（`opi_switch_language` 保缓冲、也不清片段）。
    func commitPendingBeforeModeSwitch() {
        let engine = OpiEngine.shared
        let pending = engine.pendingText()
        guard !pending.isEmpty else { return }
        engine.clear()
        commit(pending)
    }

    /// 自绘虚拟键盘的键 → 引擎（不合成系统按键，见 InputController.virtualKey）。
    @discardableResult
    func virtualKey(keyval: UInt32, states: UInt32) -> Bool {
        controller?.virtualKey(keyval: keyval, states: states) ?? false
    }

    /// 菜单/状态栏/设置页改状态后的统一刷新点：preedit/候选 + 状态栏标题。
    func refreshState() {
        controller?.refreshFromOutside()
        OpiStatusBar.shared.refresh()
    }
}

// ---------- 菜单 ----------

enum OpiMenuTag: Int {
    case toggleEnglish = 1
    case toggleTraditional
    case toggleChinesePunct
    case toggleFullwidth
    case toggleStatusBar
    case toggleVirtualKeyboard
    case schemeFull
    case schemeShuangpin
    case schemeNine
    case schemeFourteen
    case schemeStroke
    case schemeWubi
    case symbolPanel
    case emojiPanel
    case settings
    case statusBarSettings
    case about
    /// 皮肤项占 `skinBase ..< skinBase + OpiSkin.allCases.count`。
    case skinBase = 100
}

enum OpiMenu {
    /// 每次打开菜单都现建：勾选状态直接读引擎（不做缓存，缓存会显示旧模式）。
    static func build(engine: OpiEngine, target: OpiInputController) -> NSMenu {
        let menu = NSMenu(title: "OPI")
        menu.autoenablesItems = false
        let bar = OpiBarSettings.load()

        menu.addItem(item("英文模式", target: target, tag: .toggleEnglish,
                          on: engine.mode() == .english))
        menu.addItem(item("繁体模式", target: target, tag: .toggleTraditional,
                          on: engine.mode() == .traditional))
        menu.addItem(.separator())
        menu.addItem(item("中文标点", target: target, tag: .toggleChinesePunct,
                          on: engine.chinesePunctEnabled()))
        menu.addItem(item("全角字符", target: target, tag: .toggleFullwidth,
                          on: engine.fullwidthState()))
        menu.addItem(item("软键盘", target: target, tag: .toggleVirtualKeyboard,
                          on: OpiKeyboardPanel.shared.isVisible))
        menu.addItem(item("显示状态栏", target: target, tag: .toggleStatusBar,
                          on: !bar.hidden))

        // MODE：六种方案与引擎 `Scheme::code` 同序；笔画/五笔没装码表时禁用并注明。
        let scheme = NSMenuItem(title: "输入方案", action: nil, keyEquivalent: "")
        let sub = NSMenu(title: "输入方案")
        sub.autoenablesItems = false
        let current = engine.schemeCode()
        let choices: [(String, OpiMenuTag, OpiScheme)] = [
            ("全拼", .schemeFull, .full),
            ("双拼", .schemeShuangpin, .shuangpin),
            ("拼音9键", .schemeNine, .nine),
            ("拼音14键", .schemeFourteen, .fourteen),
            ("笔画", .schemeStroke, .stroke),
            ("五笔", .schemeWubi, .wubi),
        ]
        for (title, tag, value) in choices {
            let ok = engine.isSchemeAvailable(value)
            let it = item(title, target: target, tag: tag, on: current == value.rawValue,
                          enabled: ok)
            if !ok { it.toolTip = "\(value.title)词典未装载" }
            sub.addItem(it)
        }
        scheme.submenu = sub
        menu.addItem(scheme)

        // 皮肤：与状态栏 LOGO 单击循环是同一份枚举/同一份落盘键。
        let skin = NSMenuItem(title: "皮肤", action: nil, keyEquivalent: "")
        let skinMenu = NSMenu(title: "皮肤")
        skinMenu.autoenablesItems = false
        for (index, value) in OpiSkin.allCases.enumerated() {
            let it = item(value.title, target: target,
                          tag: OpiMenuTag.skinBase.rawValue + index,
                          on: OpiTheme.skin == value)
            skinMenu.addItem(it)
        }
        skin.submenu = skinMenu
        menu.addItem(skin)

        menu.addItem(.separator())
        // 快捷键只写在标题里，不设 keyEquivalent：IMK 的 handle(_:client:) 已经
        // 截获这些组合，菜单再注册一份会造成「菜单开着时按一次执行两次」。
        menu.addItem(item("符号面板…  ⌃⇧Z", target: target, tag: .symbolPanel))
        menu.addItem(item("表情与贴纸…", target: target, tag: .emojiPanel))
        menu.addItem(item("状态栏设置…", target: target, tag: .statusBarSettings))
        menu.addItem(item("设置…", target: target, tag: .settings))
        menu.addItem(.separator())
        let about = item("关于 OPI", target: target, tag: .about)
        // 菜单也带上项目宠物（与状态栏 LOGO、窗口「关于」页同一份 idle.png）。
        about.image = petImage()
        menu.addItem(about)
        return menu
    }

    /// 菜单图标（16pt）。资源随包：`Resources/pet/stickers/idle.png`。
    private static func petImage() -> NSImage? {
        guard let url = Bundle.main.url(forResource: "idle", withExtension: "png",
                                        subdirectory: "pet/stickers"),
              let image = NSImage(contentsOf: url) else { return nil }
        image.size = NSSize(width: 16, height: 16)
        return image
    }

    /// 分派菜单动作。`controller` 用来在模式变化后刷新 preedit/候选。
    static func perform(tag rawTag: Int, controller: OpiInputController) {
        let base = OpiMenuTag.skinBase.rawValue
        if rawTag >= base, rawTag < base + OpiSkin.allCases.count {
            OpiTheme.skin = OpiSkin.allCases[rawTag - base]
            OpiTheme.notifyChanged()
            return
        }
        guard let tag = OpiMenuTag(rawValue: rawTag) else { return }
        let engine = OpiEngine.shared
        switch tag {
        case .toggleEnglish:
            // 与 Ctrl+' 同判（InputController.hotkey 的 toggleEnglish 支）。
            OpiClientHub.shared.commitPendingBeforeModeSwitch()
            engine.switchMode(engine.mode() == .english ? .pinyin : .english)
            controller.refreshFromOutside()
        case .toggleTraditional:
            // 与 `opi_mode.h` 的 opiToggleTraditional 同判：在繁体就回拼音，否则进繁体。
            // 成对切换走保缓冲的 `opi_switch_language`（同 iOS `toggleTraditional`）。
            engine.switchLanguage(engine.mode() == .traditional ? .pinyin : .traditional,
                                  engine.currentScheme())
            controller.refreshFromOutside()
        case .toggleChinesePunct:
            // 只翻一个偏好位，不动缓冲/候选 —— 不需要刷新。
            engine.toggleChinesePunct()
        case .toggleFullwidth:
            engine.toggleFullwidth()
        case .toggleStatusBar:
            OpiStatusBar.shared.toggleHidden()
        case .toggleVirtualKeyboard:
            OpiKeyboardPanel.shared.toggle()
        case .schemeFull:
            OpiSettingsStore.applyScheme(.full)
            controller.refreshFromOutside()
        case .schemeShuangpin:
            OpiSettingsStore.applyScheme(.shuangpin)
            controller.refreshFromOutside()
        case .schemeNine:
            OpiSettingsStore.applyScheme(.nine)
            controller.refreshFromOutside()
        case .schemeFourteen:
            OpiSettingsStore.applyScheme(.fourteen)
            controller.refreshFromOutside()
        case .schemeStroke:
            OpiSettingsStore.applyScheme(.stroke)
            controller.refreshFromOutside()
        case .schemeWubi:
            OpiSettingsStore.applyScheme(.wubi)
            controller.refreshFromOutside()
        case .symbolPanel:
            OpiSymbolPanel.shared.show()
        case .emojiPanel:
            OpiEmojiPanel.shared.show()
        case .settings:
            OpiSettingsWindow.shared.show()
        case .statusBarSettings:
            OpiSettingsWindow.shared.show(page: .statusbar)
        case .about:
            OpiSettingsWindow.shared.show(page: .about)
        case .skinBase:
            break
        }
        OpiStatusBar.shared.refresh()
    }

    private static func item(_ title: String, target: OpiInputController,
                             tag: OpiMenuTag, on: Bool = false,
                             enabled: Bool = true) -> NSMenuItem {
        item(title, target: target, tag: tag.rawValue, on: on, enabled: enabled)
    }

    private static func item(_ title: String, target: OpiInputController,
                             tag: Int, on: Bool = false,
                             enabled: Bool = true) -> NSMenuItem {
        // target 明确指向控制器：action 直接落到 menuCommand(_:)，不赌 IMK 的
        // responder 链。doCommand 的覆写仍保留（部分 IMK 版本会把菜单动作转成它）。
        let it = NSMenuItem(title: title,
                            action: #selector(OpiInputController.menuCommand(_:)),
                            keyEquivalent: "")
        it.target = target
        it.tag = tag
        it.state = on ? .on : .off
        it.isEnabled = enabled
        return it
    }
}
