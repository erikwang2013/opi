// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 组合串一族（**扩展函数**：500 行门禁从 EngineController.kt 搬出，本仓同惯例）。
//
// 为什么是扩展而不是另立一个类：它们读的就是控制器的三个状态
// （`buffer` / `reading` / `inputScheme`，getter 都是公开的），
// **语义上仍是那个类的一部分** —— 换一个类会让「谁持有状态」多出一层。
// 调用点一字不变（`ctrl.composingText()`）。
//
// ⚠️ 两个视图别合并：`composingText`（显示，带「·」待编标记）与 `pendingCommitText`
// （**上屏**，不带）—— 合并过一次，被既有的切英文门禁抓住（见 pendingCommitText 头注）。

package xyz.erik.opi.engine

    /**
     * 推给字段的组合串：读数优先（9 键 = `ni'hao` 这类分词拼音；其余 = 原字母）；
     * 读数空时回退缓冲 —— **9 键例外**：它的缓冲是数字串（按键编码），读数空
     * （无匹配）还回退就会把数字画进字段（2026-10-11 用户报障「9 键没匹配的时候
     * 会变成数字输入」）。9 键读数空 ⇒ **仍不落数字**（见下）。
     *
     * ⚠️ 2026-10-12 晚用户报障「多次点 mno → 点候选确认 → 点 jkl 就无匹，点其他键
     * 也无响应」的根因就在本函数的两条边界上 —— 以前**待编的键完全不进字段**：
     *  1. 读数空（无匹配）⇒ 返回空串 ⇒ 字段里一个字都不动（无匹串实测占 9 键数字串
     *     的 26%：`44`/`55`/`45` 这类本就查不到），用户判为「键盘死了」；
     *  2. 读数只覆盖**候选命中的前缀**（9 键的候选是「最长可匹配前缀」那一档），
     *     缓冲里没被覆盖的尾巴（连点 6 次 `6` 再点候选确认后剩 `66`；20 次剩 16 个）
     *     同样不显示 ⇒ 再按新键时读数与候选都**一字不变** ⇒ 看着像没响应。
     * 现在**每个待编键补一个「·」**：不给数字（守 2026-10-11 那条裁决），也不编假
     * 拼音（那会像真读音、比不显示更坏）。有读数时补在读数后面（`mo'mo··`）。
     */
internal fun EngineController.composingText(): String = pendingCommitText() + pendingKeyMark()

    /**
     * **上屏用**的 pending 文本：与 [composingText] 同源，但**不含「·」标记** ——
     * 那是显示记号不是文本，跟着上屏会往字段里写点（2026-10-12 加标记时真被门禁
     * 抓到过：`ImeStateEnglishSwitchTest.switchToEnglishWithUnmatchedNineKeyDigitsCommitsNothing`）。
     * 9 键读数空 ⇒ 空串（守 2026-10-11「别变成数字输入」那条）。
     */
internal fun EngineController.pendingCommitText(): String {
        if (reading.isNotEmpty()) return reading
        return if (inputScheme == InputScheme.NINE) "" else buffer
    }

    /**
     * 待编键的标记：「·」× 缓冲里**没被读数覆盖**的键数。
     *
     * 覆盖率按**字母数**算（9/14 键每个键恰好一个字母：9 键 = 每字母一个数字，
     * 14 键 = 缓冲本就是字母）—— 读数的 `'` 是分词符，不占键位。
     * 非 9/14 键（缓冲即显示文本）恒空串；14 键的读数空时也不补（它的缓冲是字母，
     * 本来就会原样显示）。
     */
private fun EngineController.pendingKeyMark(): String {
        if (inputScheme != InputScheme.NINE && inputScheme != InputScheme.FOURTEEN) return ""
        val covered = reading.count { it != '\'' }
        val pending = buffer.length - covered
        return if (pending > 0) "·".repeat(pending) else ""
    }
