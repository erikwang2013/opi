// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

/**
 * 英文键盘底行 `pro` 键（2026-10-19）：普通 ⇄ 专业。
 *
 * 切换前清空缓冲/候选（模式变了，旧英文串不再有同一套语义）；落盘。
 * 普通 = 字母直出、不查 `en_words.tsv`；专业 = 现有前缀联想。
 */
fun ImeState.toggleEnglishPro() {
    if (controller.buffer.isNotEmpty() || controller.pendingText.isNotEmpty()) controller.clear()
    englishPro = !englishPro
    persist()
}
