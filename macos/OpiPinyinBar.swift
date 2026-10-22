// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未在 macOS 上编译/运行（见 macos/README.md 头部）
//
// OpiPinyinBar —— 候选窗上方的拼音提示条（2026-10-23，用户要求「输入时拼音不许
// 消失」）。IMKCandidates 没有标题栏，客户端 marked text 只在宿主输入框里 —— 一旦
// 宿主把 marked text 画得很淡、或候选窗翻到上面，拼音就没有独立可见的落点。
//
// 做法：**非激活、无边框、鼠标穿透**的 NSPanel，只画一行拼音。
//   · `styleMask` 含 `.nonactivatingPanel` + `becomesKeyOnlyIfNeeded`：点它不抢焦点
//     （本来就 `ignoresMouseEvents`，点击会落到下面的宿主窗口）。
//   · 位置：优先在 **光标矩形上方 / 候选窗上方**；都放不下才退到光标下方。
//   · 生命周期由 `OpiInputController.refresh` 驱动：缓冲空即 `hide()`。
//
// ⚠️ 未验证 API（本机无 Swift/macOS，CI 只 `-typecheck`）：
//   1. `IMKTextInput.attributes(forCharacterIndex:lineHeightRectangle:)` 的矩形是否
//      真是**屏幕坐标**（文档如此；若不是，提示条会错位 —— 退路见 `place`）；
//   2. IMKCandidates 的窗口层级（用 `.floating`；它若更高，提示条可能被压住，
//      此时改动 `panel.level` 即可，不影响输入逻辑）；
//   3. 多屏/全屏 Space 下的 `visibleFrame` 行为（用 `.canJoinAllSpaces` +
//      `.fullScreenAuxiliary` 兜底，仍未上机）。
// 这几条都只影响**提示条位置/层级**，不影响输入、上屏与既有 marked text。

import AppKit
import InputMethodKit

final class OpiPinyinBar {
    static let shared = OpiPinyinBar()

    private var panel: NSPanel?
    private var label: NSTextField?
    private var text = ""

    private init() {}

    /// 显示用串（自动分词）与原文二选一：显示串为空（9 键无读数）时回退原文。
    func update(display: String, raw: String, client: IMKTextInput?, candidatesFrame: NSRect?) {
        update(display.isEmpty ? raw : display, client: client, candidatesFrame: candidatesFrame)
    }

    /// 刷新一行拼音。[client] 用来取光标矩形；[candidatesFrame] = IMKCandidates 可见时
    /// 的窗口矩形（屏幕坐标），没有候选窗时传 nil。空串 = 收起。
    func update(_ newText: String, client: IMKTextInput?, candidatesFrame: NSRect?) {
        guard !newText.isEmpty else {
            hide()
            return
        }
        let p = panel ?? makePanel()
        if newText != text {
            text = newText
            layout(p, text: newText)
        }
        place(p, client: client, candidatesFrame: candidatesFrame)
        if !p.isVisible { p.orderFront(nil) }
    }

    func hide() {
        text = ""
        panel?.orderOut(nil)
    }

    // ---------- 构建 ----------

    private func makePanel() -> NSPanel {
        let p = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 120, height: 24),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false,
        )
        p.isFloatingPanel = true
        p.becomesKeyOnlyIfNeeded = true
        p.level = .floating
        p.isOpaque = false
        p.backgroundColor = .clear
        p.hasShadow = true
        p.ignoresMouseEvents = true
        p.isReleasedWhenClosed = false
        p.hidesOnDeactivate = false
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]

        let container = NSView(frame: .zero)
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.97).cgColor
        container.layer?.cornerRadius = 6
        container.layer?.borderWidth = 1
        container.layer?.borderColor = NSColor.separatorColor.cgColor

        let l = NSTextField(labelWithString: "")
        l.font = .systemFont(ofSize: 13, weight: .medium)
        l.textColor = .labelColor
        l.lineBreakMode = .byTruncatingTail
        l.drawsBackground = false
        l.setAccessibilityLabel("输入拼音")
        container.addSubview(l)

        p.contentView = container
        panel = p
        label = l
        return p
    }

    /// 手算尺寸（不用 Auto Layout：无 Mac 可验的约束面越小越稳）。
    private func layout(_ p: NSPanel, text: String) {
        guard let container = p.contentView, let l = label else { return }
        let padX: CGFloat = 8
        let padY: CGFloat = 4
        let maxTextWidth: CGFloat = 480
        let attrs: [NSAttributedString.Key: Any] = [.font: l.font ?? NSFont.systemFont(ofSize: 13)]
        let measured = (text as NSString).size(withAttributes: attrs)
        let textWidth = min(measured.width, maxTextWidth)
        let size = NSSize(width: textWidth + padX * 2, height: measured.height + padY * 2)
        p.setContentSize(size)
        container.frame = NSRect(origin: .zero, size: size)
        l.frame = NSRect(x: padX, y: padY, width: textWidth, height: measured.height)
        l.stringValue = text
    }

    // ---------- 定位 ----------

    /// 优先放在候选窗上方；没有候选窗就放在光标上方；都越界才退到光标下方。
    /// 坐标全是屏幕坐标（与 `lineHeightRectangle` 的文档口径一致）。
    private func place(_ p: NSPanel, client: IMKTextInput?, candidatesFrame: NSRect?) {
        let size = p.frame.size
        let caret = caretRect(client)
        let screen = (candidatesFrame ?? caret)
            .flatMap { anchor in NSScreen.screens.first { $0.frame.intersects(anchor) } }
            ?? NSScreen.main
        let visible = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let gap: CGFloat = 4

        var origin: NSPoint
        if let frame = candidatesFrame {
            // 候选窗在光标下方时，提示条放到**光标上方**（两者之间隔着输入行，互不遮挡）。
            origin = NSPoint(x: frame.minX, y: frame.maxY + gap)
            if let caret = caret, caret.minY >= frame.maxY {
                origin = NSPoint(x: caret.minX, y: caret.maxY + gap)
            }
        } else if let caret = caret {
            origin = NSPoint(x: caret.minX, y: caret.maxY + gap)
        } else {
            // 取不到光标（部分宿主不实现 attributes）：贴屏幕右上，至少拼音可见。
            origin = NSPoint(x: visible.maxX - size.width - 20, y: visible.maxY - size.height - 20)
        }
        // 上方放不下 → 退到光标下方；仅在候选窗不可见时这么做（避免压住候选窗）。
        if origin.y + size.height > visible.maxY, candidatesFrame == nil, let caret = caret {
            origin.y = caret.minY - size.height - gap
        }
        origin.x = max(visible.minX + gap, min(origin.x, visible.maxX - size.width - gap))
        origin.y = max(visible.minY + gap, min(origin.y, visible.maxY - size.height - gap))
        p.setFrameOrigin(origin)
    }

    /// 光标所在行矩形（屏幕坐标）。宿主没实现/给空矩形 → nil。
    private func caretRect(_ client: IMKTextInput?) -> NSRect? {
        guard let client = client else { return nil }
        var rect = NSRect.zero
        _ = client.attributes(forCharacterIndex: 0, lineHeightRectangle: &rect)
        return (rect.width > 0 || rect.height > 0) ? rect : nil
    }
}
