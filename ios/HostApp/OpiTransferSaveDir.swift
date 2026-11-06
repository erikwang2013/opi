// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// iOS 接收目录规则（从 OpiTransferService 拆出守 500 行）：
// 沙盒里只能落在 Documents/opi 下，用户可再填一层相对子目录。

import Foundation

func opiTransferSaveSubdir() -> String {
    (UserDefaults.standard.string(forKey: "TransferSaveSubdir") ?? "")
        .trimmingCharacters(in: .whitespacesAndNewlines)
}

/// nil = 可用。禁止 . / .. 与反斜杠转义。
func opiTransferSaveSubdirError(_ raw: String) -> String? {
    let t = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    if t.isEmpty { return nil }
    if t.count > 80 { return "子目录最多 80 个字符" }
    let norm = t.replacingOccurrences(of: "\\", with: "/")
        .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    if norm.isEmpty { return nil }
    if norm.split(separator: "/").contains(where: { $0.isEmpty || $0 == "." || $0 == ".." }) {
        return "子目录不能包含 . 或 .."
    }
    return nil
}

func opiTransferSetSaveSubdir(_ raw: String) -> String? {
    if let error = opiTransferSaveSubdirError(raw) { return error }
    UserDefaults.standard.set(
        raw.trimmingCharacters(in: .whitespacesAndNewlines),
        forKey: "TransferSaveSubdir"
    )
    return nil
}
