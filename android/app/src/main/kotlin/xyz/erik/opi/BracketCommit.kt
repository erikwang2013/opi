// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 括号配对的**宿主编**（2026-10-26 从 `OpiImeService.kt` 拆出守 500 行门禁）。
//! 纯状态机在 `ime/BracketPairing.kt`；本文件只做 InputConnection 读写。
//! 表与语义的规范见 `shared/brackets/README.md`（iOS/鸿蒙逐项镜像）。

package xyz.erik.opi

import android.os.Build
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import xyz.erik.opi.ime.BracketHost
import xyz.erik.opi.ime.BracketPair

internal class AndroidBracketHost(private val service: OpiImeService) : BracketHost {

    override fun textBefore(length: Int): String? = readable { ic ->
        ic.getTextBeforeCursor(length, 0)?.toString()
    }

    override fun textAfter(length: Int): String? = readable { ic ->
        ic.getTextAfterCursor(length, 0)?.toString()
    }

    /**
     * 插入成对符号并把光标移回中间。
     *
     * 起始位置取 `getExtractedText().startOffset + selectionStart`（SDK 契约：selectionStart
     * 是相对抽取文本起点的下标，真实位置要加 startOffset），失败再用 `getSelectionStart()`。
     * `setSelection` 在不支持定位的编辑器上返回 false —— 此时配对已经进了文档，
     * 光标停在闭符号之后：仍然算「已处理」，只是后续跳过/成对退格不会触发（不猜位置）。
     * 表里每个符号都是一个 UTF-16 单元，所以 `+1` 不会劈开代理对。
     */
    override fun insertPair(pair: BracketPair): Boolean {
        val ic = service.currentInputConnection ?: return false
        if (!pairingAllowed(ic)) return false
        service.dropComposing(ic)
        val caret = caret(ic)
        ic.beginBatchEdit()
        return try {
            val committed = ic.commitText(pair.open + pair.close, 1)
            if (committed && caret >= 0) ic.setSelection(caret + 1, caret + 1)
            committed
        } finally {
            ic.endBatchEdit()
        }
    }

    override fun skipClose() {
        val ic = service.currentInputConnection ?: return
        val caret = caret(ic)
        if (caret >= 0) ic.setSelection(caret + 1, caret + 1)
    }

    /** 光标夹在 open|close 中间：两侧各删一个 UTF-16 单元。 */
    override fun deletePair(): Boolean {
        val ic = service.currentInputConnection ?: return false
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            ic.deleteSurroundingTextInCodePoints(1, 1)
        } else {
            ic.deleteSurroundingText(1, 1)
        }
    }

    /** 密码/安全字段不做配对；有选区时也不做（配对会替换选区，语义由普通提交负责）。 */
    private fun pairingAllowed(ic: InputConnection): Boolean {
        val info = service.currentInputEditorInfo ?: return false
        if (isPasswordEditor(info.inputType)) return false
        // InputConnection 没有 getSelectionStart/End（那是 BaseInputConnection 的方法）；
        // 选区只有 getExtractedText 这一条通用读法，读不到就不配对（保守）。
        val et = ic.getExtractedText(ExtractedTextRequest(), 0) ?: return false
        return et.selectionStart >= 0 && et.selectionStart == et.selectionEnd
    }

    private inline fun readable(read: (InputConnection) -> String?): String? {
        val ic = service.currentInputConnection ?: return null
        if (!pairingAllowed(ic)) return null
        return read(ic)
    }

    /** 当前光标绝对下标；选区非空/宿主不支持时 -1。 */
    private fun caret(ic: InputConnection): Int {
        val et = ic.getExtractedText(ExtractedTextRequest(), 0)
        if (et != null && et.selectionStart >= 0 && et.selectionStart == et.selectionEnd) {
            return et.startOffset + et.selectionStart
        }
        return -1
    }
}
