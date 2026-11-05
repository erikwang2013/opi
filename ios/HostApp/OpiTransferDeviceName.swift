// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 传输设备名规则（从 OpiTransferService 拆出守 500 行）：
// 只允许中文、英文字母、数字；系统默认名先清洗，绝不把空格/连字符带进广播。

import Foundation
import UIKit

func opiTransferValidDeviceName(_ text: String) -> Bool {
    if text.isEmpty || text.utf8.count > 64 { return false }
    return text.unicodeScalars.allSatisfy { scalar in
        let v = scalar.value
        return (v >= 48 && v <= 57) || (v >= 65 && v <= 90) || (v >= 97 && v <= 122)
            || (v >= 0x3400 && v <= 0x4DBF) || (v >= 0x4E00 && v <= 0x9FFF)
            || (v >= 0xF900 && v <= 0xFAFF) || (v >= 0x20000 && v <= 0x2FA1F)
    }
}

func opiTransferDeviceNameError(_ text: String) -> String? {
    let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
    if t.isEmpty { return nil }
    if t.utf8.count > 64 { return "最多 64 字节（中文约 21 个字）" }
    if !opiTransferValidDeviceName(t) { return "只能使用中文、英文字母和数字，不能有空格或符号" }
    return nil
}

private func opiTransferSanitizeDeviceName(_ raw: String) -> String {
    var out = ""
    var bytes = 0
    for scalar in raw.unicodeScalars {
        let v = scalar.value
        let allowed = (v >= 48 && v <= 57) || (v >= 65 && v <= 90) || (v >= 97 && v <= 122)
            || (v >= 0x3400 && v <= 0x4DBF) || (v >= 0x4E00 && v <= 0x9FFF)
            || (v >= 0xF900 && v <= 0xFAFF) || (v >= 0x20000 && v <= 0x2FA1F)
        if !allowed { continue }
        let n = String(scalar).utf8.count
        if bytes + n > 64 { break }
        out.unicodeScalars.append(scalar)
        bytes += n
    }
    return out
}

func opiTransferLocalDeviceName() -> String {
    let custom = (UserDefaults.standard.string(forKey: "DeviceName") ?? "")
        .trimmingCharacters(in: .whitespacesAndNewlines)
    if opiTransferValidDeviceName(custom) { return custom }
    let clean = opiTransferSanitizeDeviceName(UIDevice.current.name)
    return clean.isEmpty ? "iPhone" : clean
}
