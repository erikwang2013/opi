// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

/**
 * 括号自动配对的状态机（纯逻辑，宿主通过 [BracketHost] 读写文档）。
 *
 * **键值表与行为契约的唯一规范在 `shared/brackets/README.md`** —— iOS
 * `BracketPairing.swift` 与鸿蒙 `BracketPairing.ets` 逐项镜像，改表三端一起改。
 *
 * 「只动我们自己插的那一对」靠两件事：`pending` 只记录上一次刚插入的配对，且每次
 * 动作都重新回读插入时保存的上下文（两侧各 [CONTEXT] 个字符）并**逐字比较**。
 * 任何别的输入都会清掉跟踪；即使跟踪没清（如光标被用户点走），上下文对不上也不会动作。
 */
data class BracketPair(val open: String, val close: String)

val BRACKET_PAIRS: List<BracketPair> = listOf(
    BracketPair("(", ")"),
    BracketPair("[", "]"),
    BracketPair("{", "}"),
    BracketPair("（", "）"),
    BracketPair("【", "】"),
    BracketPair("《", "》"),
    BracketPair("「", "」"),
    BracketPair("『", "』"),
    BracketPair("“", "”"),
    BracketPair("‘", "’"),
)

/** 上下文回读长度：配对符自身之外，两侧各留 8 个字符。 */
const val BRACKET_CONTEXT = 8

/** 宿主能力：读光标两侧文本、插入配对、移动/删除。单位 = 宿主原生字符串单元。 */
interface BracketHost {
    /** 光标前最多 [length] 个单元；宿主不能定位/有选区/不应配对时返回 null。 */
    fun textBefore(length: Int): String?

    /** 光标后最多 [length] 个单元；同上。 */
    fun textAfter(length: Int): String?

    /** 插入「开+闭」并把光标停在两者之间；返回 false = 没插入，调用方走普通提交。 */
    fun insertPair(pair: BracketPair): Boolean

    /** 光标右移一格（跳过已存在的闭符号）。 */
    fun skipClose()

    /** 删掉光标两侧的空配对；返回 false = 没删成，调用方退回普通退格。 */
    fun deletePair(): Boolean
}

class BracketPairing(private val host: BracketHost) {
    private data class Pending(val pair: BracketPair, val before: String, val after: String)

    private var pending: Pending? = null

    /**
     * 一个「用户敲出来的文本」进文档前的裁决。返回 true = 已处理（不要再提交一次）。
     * 非配对字符、或闭符号但不在我们插入的配对中间：清跟踪后返回 false。
     */
    fun commitTyped(text: String): Boolean {
        val opening = BRACKET_PAIRS.firstOrNull { it.open == text }
        if (opening != null) {
            pending = null
            if (!host.insertPair(opening)) return false
            val before = host.textBefore(opening.open.length + BRACKET_CONTEXT)
            val after = host.textAfter(opening.close.length + BRACKET_CONTEXT)
            // 光标定位失败时 before/after 不会正好夹住配对 —— 不跟踪，后续动作退回普通路径。
            pending = if (before != null && after != null &&
                before.endsWith(opening.open) && after.startsWith(opening.close)
            ) {
                Pending(opening, before, after)
            } else {
                null
            }
            return true
        }
        val closing = BRACKET_PAIRS.firstOrNull { it.close == text }
        val p = pending
        if (closing != null && p != null && p.pair == closing && stillBetween(p)) {
            pending = null
            host.skipClose()
            return true
        }
        pending = null
        return false
    }

    /** ⌫：光标仍在自动插入的空配对中间时删一对；返回 true = 已删。 */
    fun backspace(): Boolean {
        val p = pending ?: return false
        pending = null
        if (!stillBetween(p)) return false
        return host.deletePair()
    }

    /** 换编辑器/换模式/任何非配对提交后调用。 */
    fun reset() {
        pending = null
    }

    private fun stillBetween(p: Pending): Boolean =
        host.textBefore(p.pair.open.length + BRACKET_CONTEXT) == p.before &&
            host.textAfter(p.pair.close.length + BRACKET_CONTEXT) == p.after
}
