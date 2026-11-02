// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 语音页的滚动容器与卡片排版，从 VoiceViewController.swift 拆出（500 行门禁）。
// 只有 UI，不碰录音/识别状态。

import UIKit

extension VoiceViewController {
    func buildScroll() {
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.alwaysBounceVertical = true
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.addSubview(stack)
        view.addSubview(scroll)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            stack.leadingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 16
            ),
            stack.trailingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -16
            ),
            stack.topAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.topAnchor, constant: 16
            ),
            stack.bottomAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24
            ),
            stack.widthAnchor.constraint(
                equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -32
            ),
        ])
    }

    func buildContent() {
        addCard(views: [
            makeLabel("语音输入", style: .largeTitle, bold: true),
            makeLabel("在小欧 App 里说话，识别结果回填到键盘。", style: .body, bold: false),
        ])

        let languageTitle = makeLabel("识别语言", style: .headline, bold: true)
        addCard(views: [languageTitle, languageControl])

        statusLabel.numberOfLines = 0
        statusLabel.font = .preferredFont(forTextStyle: .headline)
        detailLabel.numberOfLines = 0
        detailLabel.font = .preferredFont(forTextStyle: .footnote)
        detailLabel.textColor = .secondaryLabel
        addCard(views: [statusLabel, detailLabel])

        micButton.setTitle("开始录音", for: .normal)
        micButton.titleLabel?.font = .preferredFont(forTextStyle: .headline)
        micButton.setTitleColor(.white, for: .normal)
        micButton.backgroundColor = .systemBlue
        micButton.layer.cornerRadius = 14
        micButton.heightAnchor.constraint(equalToConstant: 60).isActive = true
        micButton.addTarget(self, action: #selector(toggleRecording), for: .touchUpInside)
        stack.addArrangedSubview(micButton)

        let resultTitle = makeLabel("识别结果", style: .headline, bold: true)
        resultView.isEditable = false
        resultView.isSelectable = true
        resultView.font = .preferredFont(forTextStyle: .body)
        resultView.backgroundColor = .secondarySystemGroupedBackground
        resultView.layer.cornerRadius = 10
        resultView.textContainerInset = UIEdgeInsets(top: 10, left: 8, bottom: 10, right: 8)
        resultView.heightAnchor.constraint(equalToConstant: 140).isActive = true
        addCard(views: [resultTitle, resultView])

        addNote(
            "识别完全在本机完成，录音不落盘、不上传。"
                + "回到输入界面后，键盘出现时会自动插入；若没有插入，可在这里长按复制。"
        )
    }

    func addCard(views: [UIView]) {
        let inner = UIStackView(arrangedSubviews: views)
        inner.axis = .vertical
        inner.spacing = 8
        inner.translatesAutoresizingMaskIntoConstraints = false
        let card = UIView()
        card.backgroundColor = .secondarySystemGroupedBackground
        card.layer.cornerRadius = 12
        card.translatesAutoresizingMaskIntoConstraints = false
        card.addSubview(inner)
        NSLayoutConstraint.activate([
            inner.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 16),
            inner.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -16),
            inner.topAnchor.constraint(equalTo: card.topAnchor, constant: 14),
            inner.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -14),
        ])
        stack.addArrangedSubview(card)
    }

    func addNote(_ text: String) {
        let label = makeLabel(text, style: .footnote, bold: false)
        label.textColor = .tertiaryLabel
        stack.addArrangedSubview(label)
    }

    func makeLabel(
        _ text: String, style: UIFont.TextStyle, bold: Bool
    ) -> UILabel {
        let label = UILabel()
        label.text = text
        label.numberOfLines = 0
        let base = UIFont.preferredFont(forTextStyle: style)
        if bold, let descriptor = base.fontDescriptor.withSymbolicTraits(.traitBold) {
            label.font = UIFont(descriptor: descriptor, size: 0)
        } else {
            label.font = base
        }
        label.adjustsFontForContentSizeCategory = true
        return label
    }
}
