// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **未编译**（本机没有 Swift 工具链；CI apple.yml 的 iOS job 会真编译）
//
// 「自有」表情映射：宠物形态 → 点击后提交的 Unicode emoji。
//
// 与 Android/桌面共用的 `shared/pet/PetEmojiMap.kt` **逐条同源**：同样的 12 个形态、
// 同样的标签与 emoji，顺序 = `PetMood` 枚举序（过滤掉不映射的 SLEEPY / DEGRADED）。
// ⚠️ 这里是**手抄的第二份**：Kotlin 那份跨端共享，Swift 够不到它；
// 改映射时两边一起改，别只改一边。
//
// ⚠️ **iOS 草案暂无小欧矢量绘制**（本仓原则：宠物几何只保留 `shared/pet/OpiPet.kt`
// 这一份实现，不新写第三份 CoreGraphics 几何）。所以表情面板的「自有」格用
// **emoji 选择器**表达：网格里画 emoji 字形 + 小标签（例：😊 笑），点击提交该 emoji。
//
// SLEEPY / DEGRADED 有意不映射 —— 它们是输入状态（不记事 / 引擎降级），
// 不是用户可挑的表情形态（同 Kotlin 侧注释）。

import Foundation

/// 一个可点的形态：无障碍标签、点击提交的 emoji。
enum PetEmojiTable {

    struct Form {
        let label: String
        let emoji: String
    }

    /// 网格里的形态列表（顺序 = `PetEmojiMap.forms`）。
    static let forms: [Form] = [
        Form(label: "笑", emoji: "😊"),
        Form(label: "欢乐", emoji: "😄"),
        Form(label: "沮丧", emoji: "😞"),
        Form(label: "哭", emoji: "😭"),
        Form(label: "惊恐", emoji: "😱"),
        Form(label: "愤怒", emoji: "😠"),
        Form(label: "撒娇", emoji: "🥺"),
        Form(label: "卖萌", emoji: "😜"),
        Form(label: "讨好", emoji: "🙏"),
        Form(label: "敲打", emoji: "🔨"),
        Form(label: "捂脸", emoji: "🤦"),
        Form(label: "吐舌头", emoji: "😛"),
    ]
}
