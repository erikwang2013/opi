// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

/**
 * 键面「紧凑字号」判据（Qwerty / 九宫格 / 数字盘共用）。
 *
 * · 矮：横屏窗口高度封顶后，键区只剩约 140dp（5 行 → 每行 28dp），21sp 行高 25dp
 *   加按键内边距会贴边；降一档到 16–17sp。
 * · 窄：单手模式键区占屏宽 76%，320dp 屏只剩 243dp（九宫格中列每格约 44dp），
 *   多字提示（"wxyz"）与右列「重输」「英文」在 21sp 下放不下。
 *
 * 常规竖屏（键区高约 220dp、宽 ≥300dp）不触发，字号与 2026-10-09 裁决一致。
 */
internal fun compactKeys(widthDp: Float, heightDp: Float): Boolean =
    widthDp < 300f || heightDp < 170f
