// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiBarIcons.swift —— 状态栏的自绘图标/字形面。
//
// 几何逐条镜像 Linux `desktop/…/BarGlyphs.kt`：
//   * FULLWIDTH = 月相：满月=实心圆，半月=描边圆 + 右半实心；
//   * VK = 圆角键帽外框 + 六个键点；
//   * SETTINGS = 2×2 四个圆角方块（缝占 0.23）；
//   * PUNCT = 「。」左上、「，」右下（英文态「.」「,」），各自在象限里按墨迹取最大；
//   * MODE/ENPRO = 主字 + 右下角标的键帽槽位（en + 26 / en + pro）。
// 面内容整体取按钮的 0.6（`FACE_SCALE`，2026-10-15 用户裁决），由调用方给 size。
//
// 只画成 NSImage，不碰事件：点击仍由 OpiHoverButton/状态栏处理。

import AppKit

enum OpiBarIcons {

    /// Linux `FACE_SCALE`。调用方传「按钮边长」，这里返回 0.6 倍的内盒图。
    static let faceScale: CGFloat = 0.6

    // ---------- 面 ----------

    static func punctPair(chinese: Bool, buttonSide: CGFloat, color: NSColor) -> NSImage {
        let face = buttonSide * faceScale
        let pair = chinese ? ("。", "，") : (".", ",")
        return image(side: face) { _ in
            let half = face / 2
            drawFitted(pair.0, in: NSRect(x: 0, y: half, width: half, height: half),
                       color: color)
            drawFitted(pair.1, in: NSRect(x: half, y: 0, width: half, height: half),
                       color: color)
        }
    }

    static func moon(fullwidth: Bool, buttonSide: CGFloat, color: NSColor) -> NSImage {
        let d = buttonSide * faceScale
        return image(side: d) { _ in
            let ctx = NSGraphicsContext.current?.cgContext
            let stroke = d * 0.09
            let r = d / 2 - stroke / 2
            let c = CGPoint(x: d / 2, y: d / 2)
            ctx?.setFillColor(color.cgColor)
            if fullwidth {
                ctx?.fillEllipse(in: CGRect(x: c.x - r, y: c.y - r,
                                            width: r * 2, height: r * 2))
            } else {
                ctx?.setStrokeColor(color.cgColor)
                ctx?.setLineWidth(stroke)
                ctx?.strokeEllipse(in: CGRect(x: c.x - r, y: c.y - r,
                                             width: r * 2, height: r * 2))
                ctx?.beginPath()
                ctx?.addArc(center: c, radius: r,
                            startAngle: -CGFloat.pi / 2,
                            endAngle: CGFloat.pi / 2, clockwise: false)
                ctx?.addLine(to: c)
                ctx?.closePath()
                ctx?.fillPath()
            }
        }
    }

    static func keyboard(buttonSide: CGFloat, color: NSColor) -> NSImage {
        let d = buttonSide * faceScale
        return image(side: d) { _ in
            let stroke = d * 0.09
            let h = d * 0.72
            let top = (d - h) / 2
            let frame = NSRect(x: stroke / 2, y: top, width: d - stroke, height: h)
            let path = NSBezierPath(roundedRect: frame,
                                    xRadius: d * 0.15, yRadius: d * 0.15)
            path.lineWidth = stroke
            color.setStroke()
            path.stroke()
            color.setFill()
            for row in 0..<2 {
                for col in 0..<3 {
                    let dotR = d * 0.065
                    let cx = d * (0.28 + CGFloat(col) * 0.22)
                    // Linux 是从顶部排；圆点布局上下对称，这里按底边坐标算同一组点。
                    let cy = top + h * (0.35 + CGFloat(1 - row) * 0.3)
                    NSBezierPath(ovalIn: NSRect(x: cx - dotR, y: cy - dotR,
                                                width: dotR * 2, height: dotR * 2)).fill()
                }
            }
        }
    }

    static func settings(buttonSide: CGFloat, color: NSColor) -> NSImage {
        let d = buttonSide * faceScale
        return image(side: d) { _ in
            let gap = d * 0.23
            let s = (d - gap) / 2
            color.setFill()
            for row in 0..<2 {
                for col in 0..<2 {
                    let rect = NSRect(x: CGFloat(col) * (s + gap),
                                      y: CGFloat(1 - row) * (s + gap),
                                      width: s, height: s)
                    NSBezierPath(roundedRect: rect, xRadius: s * 0.2,
                                 yRadius: s * 0.2).fill()
                }
            }
        }
    }

    /// 模式/ENPRO 面：主字 +（可选）右下角标。槽位按 Linux `keycapMainSlot` /
    /// `keycapCornerSlot`（左上原点，转成本函数的底左原点）。
    static func modeFace(main: String, corner: String?, buttonSide: CGFloat,
                         color: NSColor) -> NSImage {
        let d = buttonSide * faceScale
        return image(side: d) { _ in
            let mainRect = slotRect(left: 0.10, top: 0.20, right: 0.74, bottom: 0.66, side: d)
            drawFitted(main, in: mainRect, color: color)
            if let corner {
                let cornerRect = slotRect(left: 0.55, top: 0.69,
                                          right: 0.93, bottom: 0.94, side: d)
                drawFitted(corner, in: cornerRect, color: color)
            }
        }
    }

    // ---------- 工具 ----------

    private static func image(side d: CGFloat,
                              draw: @escaping (CGFloat) -> Void) -> NSImage {
        NSImage(size: NSSize(width: d, height: d), flipped: false) { _ in
            draw(d)
            return true
        }
    }

    /// 左上原点的比例槽位 → 底左原点的像素矩形。
    private static func slotRect(left: CGFloat, top: CGFloat, right: CGFloat,
                                 bottom: CGFloat, side: CGFloat) -> NSRect {
        NSRect(x: left * side, y: (1 - bottom) * side,
               width: (right - left) * side, height: (bottom - top) * side)
    }

    /// 在盒里按墨迹（用步进盒近似）取最大字号并居中。
    private static func drawFitted(_ text: String, in box: NSRect, color: NSColor) {
        guard !text.isEmpty, box.width > 0, box.height > 0 else { return }
        var size = max(box.width, box.height)
        var font = NSFont.systemFont(ofSize: size)
        for _ in 0..<8 {
            let measured = (text as NSString).size(withAttributes: [.font: font])
            guard measured.width > 0, measured.height > 0 else { break }
            let k = min(box.width / measured.width, box.height / measured.height)
            if abs(k - 1) < 0.02 { break }
            size = max(1, size * k)
            font = NSFont.systemFont(ofSize: size)
        }
        let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: color]
        let measured = (text as NSString).size(withAttributes: attrs)
        let point = NSPoint(x: box.midX - measured.width / 2,
                            y: box.midY - measured.height / 2)
        (text as NSString).draw(at: point, withAttributes: attrs)
    }
}
