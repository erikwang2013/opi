// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// OPI iOS 宿主 App —— 只为承载键盘扩展（Apple 规定扩展不能独立安装）。
//
// 仓库仍然不手写 .xcodeproj：`scripts/apple/package-ios.sh` 用 swiftc 把这个文件
// 直接编成 `Opi.app/Opi`，与扩展一起组装进 `Payload/Opi.app` 的 unsigned IPA。
// 没有 storyboard / nib，窗口与说明文字全部代码构建。

import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        let window = UIWindow(frame: UIScreen.main.bounds)
        let controller = UIViewController()
        controller.view.backgroundColor = .systemBackground

        let title = UILabel()
        title.text = "小欧输入法"
        title.font = .preferredFont(forTextStyle: .largeTitle)
        title.textAlignment = .center
        title.adjustsFontForContentSizeCategory = true

        let steps = UILabel()
        steps.numberOfLines = 0
        steps.font = .preferredFont(forTextStyle: .body)
        steps.textAlignment = .center
        steps.text = """
        设置 → 通用 → 键盘 → 键盘
        → 添加新键盘 → 小欧输入法

        本输入法离线运行，默认不需要网络；
        在密码输入框中系统会强制使用内置键盘。
        """

        let stack = UIStackView(arrangedSubviews: [title, steps])
        stack.axis = .vertical
        stack.spacing = 24
        stack.translatesAutoresizingMaskIntoConstraints = false
        controller.view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: controller.view.readableContentGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: controller.view.readableContentGuide.trailingAnchor),
            stack.centerYAnchor.constraint(equalTo: controller.view.centerYAnchor),
        ])

        window.rootViewController = controller
        window.makeKeyAndVisible()
        self.window = window
        return true
    }
}
