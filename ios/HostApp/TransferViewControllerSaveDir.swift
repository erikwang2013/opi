// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 传输页的「接收目录」卡片（从 TransferViewController 拆出守 500 行）。

import UIKit

extension TransferViewController {
    func addSaveDirCard() {
        let field = UITextField()
        field.text = opiTransferSaveSubdir()
        field.placeholder = "Documents/opi 下的子目录，留空用默认"
        field.borderStyle = .roundedRect
        field.autocapitalizationType = .none
        field.autocorrectionType = .no
        field.clearButtonMode = .whileEditing
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let save = makeSmallButton("保存目录") { [weak self, weak field] in
            guard let self, let field else { return }
            if let error = opiTransferSetSaveSubdir(field.text ?? "") {
                self.alert(error)
            } else {
                self.reload()
            }
        }
        addCard([
            makeLabel("接收目录", style: .headline),
            makeRow([field, save], equal: false),
            makeNote("接收文件落在 Documents/opi/<子目录>/<传输 ID 前 8 位>；留空用 Documents/opi。"
                     + "iOS 沙盒不允许任意绝对路径。"),
        ])
    }
}
