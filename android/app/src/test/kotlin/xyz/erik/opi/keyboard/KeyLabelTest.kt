// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import xyz.erik.opi.candidate.composingAnnouncement
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.InputScheme
import xyz.erik.opi.ime.jianFanModeKeyLabelOf
import xyz.erik.opi.ime.modeKeyLabelOf
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 读屏播报文案（纯函数，JVM 可测）。
 * Compose 语义本身（liveRegion / stateDescription 是否真被 TalkBack 念出来）要真机 + 读屏软件，
 * 本机无设备测不了 —— 手工验收步骤见交付说明。
 */
class KeyLabelTest {

    @Test
    fun glyphKeysGetReadableNames() {
        assertEquals("大写", spokenKeyName("⇧"))
        assertEquals("退格", spokenKeyName("⌫"))
        assertEquals("回车", spokenKeyName("↵"))
        assertEquals("空格", spokenKeyName("空格"))
        assertEquals("中英切换", spokenKeyName("中"))
        assertEquals("中英切换", spokenKeyName("繁"))
        assertEquals("中英切换", spokenKeyName("英"))
        assertEquals("数字与符号", spokenKeyName("123"))
        assertEquals("数字与符号", spokenKeyName("?123"))
        assertEquals("字母", spokenKeyName("ABC"))
    }

    @Test
    fun letterKeysGetSuffix() {
        assertEquals("a 键", spokenKeyName("a"))
        assertEquals("m 键", spokenKeyName("m"))
    }

    @Test
    fun digitsAndPunctuationReadAsIs() {
        assertEquals("1", spokenKeyName("1"))
        assertEquals("逗号", spokenKeyName(","))
        assertEquals("句号", spokenKeyName("."))
    }

    @Test
    fun composingAnnouncementCountsCandidates() {
        assertEquals("拼音 nihao，5 个候选", composingAnnouncement("nihao", 5))
        assertEquals("拼音 nihao，无候选", composingAnnouncement("nihao", 0))
        // 2026-10-14：五笔态那格放的是码串 —— 念成「编码」，别把码串说成拼音。
        assertEquals("编码 wqvb，3 个候选", composingAnnouncement("wqvb", 3, label = "编码"))
    }

    /**
     * 9 键的缓冲格**恒空**（2026-10-09 用户二次裁决：「输入无匹配的时候，候选出现
     * logo 图标，logo 图标前就不要显示数字了」）—— 别把「无候选时显示缓冲原文」
     * 加回来（曾被当作反馈修复，用户明确不要）。
     */
    // ⚠️ 本条 2026-10-15 撤除：`barBufferText` 随「拼音不进字段、改画在候选行上方」
    // （用户裁决）一并删除 —— 拼音行读的是 `EngineController.composingText()`，
    // 那条的取值（9 键不落数字、读数优先）仍由 `EngineControllerComposingTest` 钉着。
    @Test
    fun announcementOmitsTheBufferWhenEmpty() {
        // 保号占位：9 键「不显示数字串」这条**没有**撤销，只是换了个出口 ——
        // 现在由 `composingText()`（ComposingBar 读它）保证，断言在 ComposingTest。
        // 选字后 buffer 清空而候选仍在：不能播成「拼音 ，3 个候选」
        assertEquals("3 个候选", composingAnnouncement("", 3))
    }

    @Test
    fun modeKeyLabelShowsTheTargetNotTheCurrentMode() {
        // 2026-10-12 晚用户裁决：「点击英文按钮切换到英文键盘，这时为何在英文键盘还显示
        // 英的按钮，点击了是中文键盘，按钮是中才对」——键面写字面目标（与 9 键底行
        // 「英文」键同一条语义）。逐条对齐 cycleMode() 的次序：中→繁、繁→英、英→回中文。
        assertEquals("繁", modeKeyLabelOf(EngineMode.PINYIN, lastChineseIsTrad = false))
        assertEquals("英", modeKeyLabelOf(EngineMode.TRADITIONAL, lastChineseIsTrad = false))
        assertEquals("中", modeKeyLabelOf(EngineMode.ENGLISH, lastChineseIsTrad = false))
        // 数字面板也走 cycleMode 的 else 档（回上次的中文方案）⇒ 键面是「中」不是「英」。
        // ⚠️ 我第一版把这条写成了「英」——是照抄旧语义（当前态一律非拼音按英文）写的，
        // 实现是对的、期望错了，跑出来红在这条（expected 英 but was 中）。
        assertEquals("中", modeKeyLabelOf(EngineMode.NUMBER, lastChineseIsTrad = false))
        // 「英→回上次的中文方案」：上次是繁体就回繁体（键面同说实话）
        assertEquals("繁", modeKeyLabelOf(EngineMode.ENGLISH, lastChineseIsTrad = true))
        assertEquals("繁", modeKeyLabelOf(EngineMode.NUMBER, lastChineseIsTrad = true))
    }

    @Test
    fun modeKeyOnlyExpressesChineseAndTraditional() {
        // 2026-10-15 用户裁决「简繁按钮就只切换简繁」，同日「统一成只切简繁」把 14 键与
        // 笔画也归到这条：这三盘上这枚键**不循环进英文**（进英文走各自底行那枚独立的
        // [英文] 键），故键面只在中/繁之间表达目标。
        // 语义仍与 26 键同一条（写「点它会切到哪」），只是把「英」那条腿摘掉。
        assertEquals("繁", jianFanModeKeyLabelOf(EngineMode.PINYIN))
        assertEquals("中", jianFanModeKeyLabelOf(EngineMode.TRADITIONAL))
        // 这三盘按 inputMode 分发、拿不到英文/数字态；兜底给「繁」：不空、不炸
        assertEquals("繁", jianFanModeKeyLabelOf(EngineMode.ENGLISH))
        assertEquals("繁", jianFanModeKeyLabelOf(EngineMode.NUMBER))
    }

    /**
     * **接线的静态门禁**（2026-10-15 用户裁决「统一成只切简繁」；14 键 2026-10-24
     * 改版后底行是「中英」，不再归这条）。
     *
     * 为什么必须静态：8 处 `when(state.inputMode)` 分支里谁接哪条 handler 是 Compose
     * 层的**调用形状**，本仓 JVM 单测够不着（无 Robolectric）；而这条裁决的要害恰恰是
     * 「该接的盘接同一条、26 键那条不许跟着改」——纯函数测（上一条）只看键面字，接线
     * 错了它照样绿。先例：`ComposingPlacementTest` 的布局次序门禁。
     *
     * ⚠️ 按**调用形状**取，不按行号（行号随任何编辑漂）。
     * ⚠️ 它证明的是「代码仍然这么写」，**不**证明真机上面板行为。
     */
    @Test
    fun twoKeyboardsBindThePlainJianFanToggleAndTheFourteenKeyTogglesLanguage() {
        val rel = "src/main/kotlin/xyz/erik/opi/ime/ImeScreen.kt"
        val p = java.nio.file.Paths.get(rel)
        assertTrue("源文件不在了（搬家了？）：$rel", java.nio.file.Files.exists(p))
        // ⚠️ readAllBytes 而不是 Files.readString（Java 11 API，本模块单测编译级别不可见）。
        val src = String(java.nio.file.Files.readAllBytes(p), Charsets.UTF_8)
        fun count(needle: String) = Regex(Regex.escape(needle)).findAll(src).count()
        // 9 / 笔画 = 两处切简繁；14 键改走中英切换；26 键 + 搜索叠盘 = 两处 cycleMode
        assertEquals("切简繁的键应恰有两盘（9/笔画）", 2, count("onModeSwitch = state::toggleTraditional"))
        assertEquals("14 键底行中英 = 一条切语言", 1, count("onModeSwitch = state::toggleChineseEnglish"))
        assertEquals("26 键与搜索叠盘的 cycleMode 不许被带上", 2, count("onModeSwitch = state::cycleMode"))
        // 独立英文入口：9/笔画两盘各一枚（14 键改版后由「中英」承担）。
        assertEquals("两盘各自的独立 [英文] 键", 2, count("onEnglish = state::switchToEnglishKeyboard"))
    }

    /**
     * 9 键弹层序号 1（分词格，2026-10-24）：Nine 协议没有 '1'，引擎对 raw `'1'`
     * 是空操作 ⇒ 宿主必须按「空缓冲分词键」直提，否则默认高亮项松手无反应。
     */
    @Test
    fun nineKeyOrdinalOneIsCommittedLiterally() {
        val src = String(
            java.nio.file.Files.readAllBytes(
                java.nio.file.Paths.get("src/main/kotlin/xyz/erik/opi/ime/ImeScreen.kt"),
            ),
            Charsets.UTF_8,
        )
        assertTrue(
            "9 键 onKey 必须把弹层序号 1 直提（Nine 无 '1' 类）",
            src.contains("if (ch == \"1\") router.commitText(\"1\")"),
        )
    }
}
