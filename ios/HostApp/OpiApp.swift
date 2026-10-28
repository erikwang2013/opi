// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// OPI iOS 宿主 App —— 承载键盘扩展（Apple 规定扩展不能独立安装），并给出
// **真实可用**的安装/设置指引（2026-10-21 用户报障「iOS 安装后点击无设置页面」）。
//
// 为什么设置主体不在这里：宿主 App 与键盘扩展**没有配置 App Group**，两者
// 各有自己的容器。宿主里的开关不可能同步到键盘（反过来也一样），所以这里
// 只做「怎么启用 + 设置在哪里 + 数据与隐私」，不摆一排会撒谎的假开关。
// 键盘内的设置面板（工具条齿轮）才是真正生效的那一份。
//
// 仓库仍然不手写 .xcodeproj：`scripts/apple/package-ios.sh` 用 swiftc 把这个文件
// 直接编成 `Opi.app/Opi`，与扩展一起组装进 `Payload/Opi.app` 的 unsigned IPA。
// 没有 storyboard / nib；整个页面是滚动容器 + 卡片，支持动态字号与横屏。

import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        let window = UIWindow(frame: UIScreen.main.bounds)
        let home = HomeViewController()
        let nav = UINavigationController(rootViewController: home)
        nav.navigationBar.prefersLargeTitles = true
        window.rootViewController = nav
        window.makeKeyAndVisible()
        self.window = window
        return true
    }
}

/// 宿主 App 唯一一屏：启用步骤 + 设置入口说明 + 数据说明 + 关于。
final class HomeViewController: UIViewController {

    private static let homeURL = "https://opi.erik.xyz"
    private static let projectURL = "https://github.com/erikwang2013/opi"
    private static let privacyURL = "https://opi.erik.xyz/privacy/index.html"

    private let scroll = UIScrollView()
    private let stack = UIStackView()

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "小欧输入法"
        view.backgroundColor = .systemGroupedBackground
        // 右上角直达系统设置。iOS **不允许** App 跳到「键盘」子页，
        // 只能到本 App 的设置页，所以卡片里另写了手动路径（不骗用户）。
        navigationItem.rightBarButtonItem = UIBarButtonItem(
            title: "系统设置", style: .plain,
            target: self, action: #selector(openSystemSettings)
        )
        buildScroll()
        buildContent()
    }

    private func buildScroll() {
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.alwaysBounceVertical = true
        // 容器已钉在安全区上；别再让系统按安全区二次缩内容（横屏会左右错位）。
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.addSubview(stack)
        view.addSubview(scroll)
        // 2026-10-21 用户报障「安装页点不动/内容不换行」：此前 stack 只钉了
        // contentLayoutGuide 的左右，没有宽度上限，UILabel 的 intrinsic width 会把
        // 内容撑到屏外（横屏/大字号更明显）。补一条「卡片区宽 = 可视宽 − 32」，
        // 并把滚动容器钉在安全区上（刘海/横屏/导航栏都不再压内容）。
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

    private func buildContent() {
        addHero()

        addCard(title: "启用键盘（三步）", body: [
            "1. 打开「设置 → 通用 → 键盘 → 键盘 → 添加新键盘」，选择「小欧输入法」。",
            "2. 在任意输入框里长按地球键，切换到「小欧输入法」。",
            "3. 第一次使用时，如系统提示，请在键盘上允许本键盘。",
        ])
        addPrimaryButton("打开系统设置", action: #selector(openSystemSettings))
        addNote(
            "iOS 不让 App 直接跳到「键盘」设置页：按钮只到系统设置首页，"
                + "再按第 1 步手动进入。"
        )

        addCard(title: "设置在哪里", body: [
            "调出小欧键盘，点顶部工具条上的齿轮（⚙，在收起键左边）。",
            "常用：学习、模糊音（逐组）、拼音纠错、简拼。",
            "按键：中文标点、双拼布局、键盘模式（单手 / 暗光 / 长辈）。",
            "外观：12 款皮肤。符号与表情：表情页「自有」贴纸开关。",
            "关于：版本、主页、项目、博客与隐私政策地址。",
        ])
        addNote(
            "宿主 App 与键盘扩展没有共享容器（未配置 App Group），两边设置不会互相同步。"
                + "真正生效的设置只在键盘内，所以这里不摆会撒谎的开关。"
        )

        addCard(title: "数据与隐私", body: [
            "输入、候选与用户词学习全部在本机完成；App 与键盘扩展都不发起网络请求。",
            "学到的词只存在键盘扩展自己的容器里；删除 App 即一并删除，"
                + "不经过任何服务器。",
            "键盘不需要「允许完全访问」即可打字；密码输入框由系统强制使用内置键盘。",
        ])

        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString")
            as? String ?? "未知"
        addCard(title: "关于", body: [
            "小欧输入法（opi）v\(version) · 回归本质、人人可用的多端输入法。",
            "代码按 MIT 许可；词库数据与代码分开授权。© 2026 erik.xyz",
        ])
        addLink(Self.homeURL)
        addLink(Self.projectURL)
        addLink(Self.privacyURL)

        let footer = makeLabel(
            "Open People Input · 开放、纯粹、跨端", style: .footnote, bold: false
        )
        footer.textAlignment = .center
        footer.textColor = .tertiaryLabel
        stack.addArrangedSubview(footer)
    }

    // MARK: - 卡片与控件

    private func addHero() {
        let icon = UIImageView(image: UIImage(systemName: "keyboard"))
        icon.contentMode = .scaleAspectFit
        icon.tintColor = .systemBlue
        icon.translatesAutoresizingMaskIntoConstraints = false
        icon.heightAnchor.constraint(equalToConstant: 44).isActive = true

        let name = makeLabel("小欧输入法", style: .largeTitle, bold: true)
        let tagline = makeLabel(
            "回归本质、人人可用的离线输入法", style: .body, bold: false
        )
        tagline.textColor = .secondaryLabel
        addCard(views: [icon, name, tagline])
    }

    private func addCard(title: String, body: [String]) {
        var views: [UIView] = [makeLabel(title, style: .headline, bold: true)]
        for line in body {
            let label = makeLabel(line, style: .body, bold: false)
            label.textColor = .secondaryLabel
            views.append(label)
        }
        addCard(views: views)
    }

    private func addCard(views: [UIView]) {
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

    private func addPrimaryButton(_ title: String, action: Selector) {
        let b = UIButton(type: .system)
        b.setTitle(title, for: .normal)
        b.titleLabel?.font = .preferredFont(forTextStyle: .headline)
        b.titleLabel?.adjustsFontForContentSizeCategory = true
        b.titleLabel?.numberOfLines = 0
        b.titleLabel?.textAlignment = .center
        b.backgroundColor = .systemBlue
        b.setTitleColor(.white, for: .normal)
        b.layer.cornerRadius = 10
        b.contentEdgeInsets = UIEdgeInsets(top: 12, left: 16, bottom: 12, right: 16)
        b.addTarget(self, action: action, for: .touchUpInside)
        stack.addArrangedSubview(b)
    }

    private func addNote(_ text: String) {
        let label = makeLabel(text, style: .footnote, bold: false)
        label.textColor = .tertiaryLabel
        stack.addArrangedSubview(label)
    }

    private func addLink(_ raw: String) {
        let b = UIButton(type: .system)
        b.setTitle(raw, for: .normal)
        b.titleLabel?.font = .preferredFont(forTextStyle: .callout)
        b.titleLabel?.adjustsFontForContentSizeCategory = true
        b.titleLabel?.numberOfLines = 0
        b.titleLabel?.textAlignment = .left
        b.contentHorizontalAlignment = .leading
        b.addAction(UIAction { _ in
            guard let url = URL(string: raw) else { return }
            UIApplication.shared.open(url, options: [:], completionHandler: nil)
        }, for: .touchUpInside)
        stack.addArrangedSubview(b)
    }

    private func makeLabel(
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

    @objc private func openSystemSettings() {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url, options: [:], completionHandler: nil)
    }
}
