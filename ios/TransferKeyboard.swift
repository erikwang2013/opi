// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 键盘侧跨设备传输入口。键盘扩展**不能**在后台发现/传输（也没有本地网络与
// 持久运行权限），所以这里只做一件事：用 URL scheme 打开宿主 App 的传输页
// （与 `VoiceKeyboard.swift` 同一模式）。发现、收发、落盘全在宿主进程。
//
// 系统拒绝打开时必须有可见提示（沿用剪贴板的 flash，不静默）。

import UIKit

extension KeyboardViewController {
    func openTransferFromKeyboard() {
        guard let url = URL(string: "opi-transfer://transfer") else { return }
        guard let context = extensionContext else {
            OpiClipboard.shared.showFlash("请打开小欧 App 使用跨设备传输")
            return
        }
        context.open(url) { [weak self] opened in
            guard !opened else { return }
            DispatchQueue.main.async {
                self?.showTransferMessage("请打开小欧 App 使用跨设备传输")
            }
        }
    }

    private func showTransferMessage(_ message: String) {
        OpiClipboard.shared.showFlash(message)
    }
}
