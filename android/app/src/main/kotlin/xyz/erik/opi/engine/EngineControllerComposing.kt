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
     * 当时的修法：**每个待编键补一个「·」**（有读数时补在读数后面，`mo'mo··`）。
     *
     * ⚠️ 2026-10-13 用户订正（「输入的时候，拼音输入会偶尔出现直接变成两个点」）：
     * **整段无匹（读数空）时一个点也不补** —— 用户按 `44` 这类串时字段被「··」占住，
     * 看着像输入被换成了怪东西；而且点多长、按几下 ⌫ 才对不上（`ni'hao` 2 键 6 个字），
     * 过量退格会顺势删掉已上屏的正文（用户同日报障「删除原来存在的输入」，模拟器
     * 实测：⌫ 多按 2 下 → 「米哈游」变「米」）。反馈载体改回**候选栏既有的「无匹配」**
     * （小欧 + 文案，条件 = 无候选且缓冲非空，见 CandidateBar）——那格本就在，
     * 字段里不必再堆记号。**读数覆盖前缀时的尾巴补点不变**（那是 10-12 那条的落点，
     * 且此时字段里有真拼音参照、不会看起来像"变成了点"）。
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
     * 非 9/14 键（缓冲即显示文本）恒空串。
     *
     * ⚠️ **读数空（整段无匹）⇒ 一个点也不补**（2026-10-13 用户订正，见 [composingText]
     * 的时间线）：9 键字段留空、反馈交给候选栏的「无匹配」；14 键的显示文本本来就是
     * 字母缓冲（`pendingCommitText` 的回退），补点只会变成假记号 —— 这条同时兑现了
     * 本函数旧头注「14 键的读数空时也不补」的承诺（旧代码漏了，14 键读数空会补满点）。
     */
private fun EngineController.pendingKeyMark(): String {
        if (inputScheme != InputScheme.NINE && inputScheme != InputScheme.FOURTEEN) return ""
        if (reading.isEmpty()) return ""
        val covered = reading.count { it != '\'' }
        val pending = buffer.length - covered
        return if (pending > 0) "·".repeat(pending) else ""
    }
