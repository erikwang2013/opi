// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 文本/剪贴板定向路由（2026-11-04）：只和选中设备收发。单独成文件守 500 行门禁。

import UIKit

extension OpiTransferService {
    /// 空串 = 全部已配对设备；非空 = 只和该设备收发文本/剪贴板。
    func setTextRoutes(peerId: String) {
        runner.setTextRoutes(sendPeer: peerId, receivePeer: peerId)
    }
}

final class OpiTransferRoutingState {
    static let shared = OpiTransferRoutingState()
    var onlySelected = false
    var peerId = ""

    func apply(to service: OpiTransferService) {
        service.setTextRoutes(peerId: onlySelected ? peerId : "")
    }
}

extension TransferViewController {
    func installRoutingToggle(in views: inout [UIView]) {
        let title = UILabel()
        title.text = "只和选中设备收发文本/剪贴板"
        title.font = .preferredFont(forTextStyle: .subheadline)
        title.numberOfLines = 0
        title.setContentHuggingPriority(.defaultLow, for: .horizontal)

        let toggle = UISwitch()
        toggle.isOn = OpiTransferRoutingState.shared.onlySelected
        toggle.addAction(UIAction { [weak self] _ in
            OpiTransferRoutingState.shared.onlySelected = toggle.isOn
            guard let self else { return }
            OpiTransferRoutingState.shared.apply(to: self.service)
        }, for: .touchUpInside)

        let row = UIStackView(arrangedSubviews: [title, toggle])
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 8
        views.append(row)
    }
}
