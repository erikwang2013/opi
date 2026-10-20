// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineMode

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

/**
 * 离开英文去中文时关闭 pro（2026-10-20 用户裁决「开启 pro 后切到中文就关闭」）。
 *
 * 只在**当前确实是英文且目标不是英文**时生效；恢复偏好（restore）时引擎已经不在英文，
 * 不能把用户保存的 pro 偏好在启动时静默改掉。落盘与 [toggleEnglishPro] 同一条。
 */
internal fun ImeState.leaveEnglishProFor(target: EngineMode) {
    if (!englishPro || controller.mode != EngineMode.ENGLISH || target == EngineMode.ENGLISH) return
    englishPro = false
    persist()
}
