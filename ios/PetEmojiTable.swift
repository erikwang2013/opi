// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **未编译**（本机没有 Swift 工具链；CI apple.yml 的 iOS job 会真编译）
//
// 「自有」表情的**标签 + 贴纸文件名**（2026-10-18 用户裁决：点形态插入图片，
// 不再提交 Unicode emoji）。
//
// 标签逐条等于 `shared/pet/PetEmojiMap.kt` 的 `label`；文件名逐条等于
// `shared/pet/PetStickerAssets.kt` 的 `stickerFile` 去掉 `.png`（小写、下划线换
// 连字符）。顺序 = `PetMood` 枚举序（过滤掉不映射的 SLEEPY / DEGRADED）。
// ⚠️ 这里是**手抄的第二份**：Kotlin 那份跨端共享，Swift 够不到它；
// 改映射时两边一起改，别只改一边。
//
// 贴纸 PNG 的字节内嵌在 `PetStickerImages.swift`（12 张 280×280，源
// `shared/pet/stickers/`）；点击只复制图片到系统剪贴板，**不提交任何文本**。

import Foundation

/// 一个可点的形态：无障碍标签、贴纸文件名（不含 `.png`）。
enum PetEmojiTable {

    struct Form {
        let label: String
        /// 与 `shared/pet/PetStickerAssets.kt` 同一规则：`face_palm` → `face-palm`。
        let sticker: String
    }

    /// 网格里的形态列表（顺序 = `PetEmojiMap.forms`）。
    static let forms: [Form] = [
        Form(label: "小欧·笑", sticker: "idle"),
        Form(label: "小欧·欢乐", sticker: "waiting"),
        Form(label: "小欧·沮丧", sticker: "puzzled"),
        Form(label: "小欧·哭", sticker: "crying"),
        Form(label: "小欧·惊恐", sticker: "terrified"),
        Form(label: "小欧·愤怒", sticker: "angry"),
        Form(label: "小欧·撒娇", sticker: "coquettish"),
        Form(label: "小欧·卖萌", sticker: "cute"),
        Form(label: "小欧·讨好", sticker: "pleasing"),
        Form(label: "小欧·敲打", sticker: "knocking"),
        Form(label: "小欧·捂脸", sticker: "face-palm"),
        Form(label: "小欧·吐舌头", sticker: "tongue-out"),
    ]
}
