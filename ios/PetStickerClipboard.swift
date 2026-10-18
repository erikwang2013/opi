// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **未编译**（2026-10-18 新增；本机没有 swiftc，待 CI apple.yml 的 iOS job）
//
// 「自有」贴纸的落点：**复制图片到系统剪贴板**。
//
// 为什么不是直接插入：iOS 键盘扩展只能通过 `textDocumentProxy.insertText` 上屏文本，
// 没有往宿主文本框插图片的接口（`UIPasteboard` 是唯一的跨应用图片通道）。所以点一格
// = 复制 + 键盘内可见提示，用户回宿主 App 自己粘贴。
//
// ⚠️ 未验证：无「完全访问」（本草案 `RequestsOpenAccess=false`）时，**写**剪贴板按
// 文档不需要完全访问（**读**才需要）；真机上第一次验的时候先看这一步，失败会表现为
// 「提示出了但粘贴不到图」。
//
// 触觉反馈在键盘扩展里可能被系统忽略（取决于设备与设置）；可见提示不依赖它。

import UIKit

enum PetStickerClipboard {

    /// 复制一格贴纸。返回 false = `PetStickerImages` 里没有这张图（内嵌数据缺失）。
    @discardableResult
    static func copy(_ form: PetEmojiTable.Form) -> Bool {
        guard let image = PetStickerImages.image(named: form.sticker) else { return false }
        UIPasteboard.general.image = image
        // 触觉只是锦上添花；扩展里被忽略也不影响复制本身。
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        UIAccessibility.post(
            notification: .announcement, argument: "已复制小欧\(form.label)贴纸，去粘贴"
        )
        return true
    }
}

/// 键盘内的可见提示（iOS 扩展没有 Toast，系统对写剪贴板也没有任何反馈）。
///
/// 用 `UILabel` 自绘内边距，避免为一个气泡再套一层容器；淡出后隐藏。
final class ToastLabel: UILabel {

    private var pending: DispatchWorkItem?

    init() {
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        font = .systemFont(ofSize: 12)
        textColor = .systemBackground
        backgroundColor = .label
        textAlignment = .center
        numberOfLines = 1
        layer.cornerRadius = 10
        clipsToBounds = true
        isHidden = true
        isUserInteractionEnabled = false
        isAccessibilityElement = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    override func drawText(in rect: CGRect) {
        super.drawText(in: rect.insetBy(dx: 10, dy: 5))
    }

    override var intrinsicContentSize: CGSize {
        let s = super.intrinsicContentSize
        return CGSize(width: s.width + 20, height: s.height + 10)
    }

    func show(_ text: String) {
        pending?.cancel()
        self.text = text
        isHidden = false
        alpha = 1
        let work = DispatchWorkItem { [weak self] in
            UIView.animate(withDuration: 0.25, animations: { self?.alpha = 0 }) { _ in
                self?.isHidden = true
            }
        }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2, execute: work)
    }
}
