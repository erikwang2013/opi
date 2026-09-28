// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.io.IOException
import java.io.StringReader

/**
 * 导入文本的**有界读**（F4）：设置页的文件选择器回调跑在主线程上，而此前是
 * `bufferedReader().readText()` —— 整份文件先变成 Kotlin String，引擎的条数上限
 * （`MAX_IMPORT_WORDS`）要到**之后**才生效。挑一个 2GB 的文件就是主线程 OOM/ANR，
 * 而且崩溃点在读盘，跟「词表格式不对」毫无关系、用户也看不懂。
 *
 * 纯函数、无夹具：只吃一个 [StringReader]。
 */
class UserWordStoreReadCapTest {

    @Test
    fun readsInputShorterThanTheCap() {
        assertEquals("""{"a":1}""", UserWordStore.readImportText(StringReader("""{"a":1}"""), 1024))
    }

    @Test
    fun acceptsInputExactlyAtTheCap() {
        // 边界：上限是「允许的最大长度」而不是「超过即拒」的哨兵 —— 恰好等于必须放行
        assertEquals("abcd", UserWordStore.readImportText(StringReader("abcd"), 4))
    }

    @Test
    fun rejectsInputOverTheCap() {
        assertNull(UserWordStore.readImportText(StringReader("abcde"), 4))
    }

    @Test
    fun rejectsInputSpanningSeveralChunks() {
        // 上限远大于单次 read 的缓冲区时，累加路径也要判得出「超了」（否则大文件静默放行，
        // 正是本方法存在的理由）。取 8500 跨过一次 8192 的读。
        val over = "x".repeat(8500)
        val exact = "y".repeat(8500)

        assertNull(UserWordStore.readImportText(StringReader(over), 8499))
        assertEquals(exact, UserWordStore.readImportText(StringReader(exact), 8500))
    }

    @Test
    fun emptyInputIsEmptyStringNotRejection() {
        // 空文件算不算有效由引擎判（serde 会拒），这里只管长度
        assertEquals("", UserWordStore.readImportText(StringReader(""), 1024))
    }

    @Test
    fun readFailurePropagates() {
        // 读失败（URI 失效 / 撤权 / 磁盘错）必须冒到 importFrom 的文案里，不在这里吞成 null：
        // null 语义是「太长」，两者混起来用户会看到「文件过大」这种假话。
        val boom = object : StringReader("x") {
            override fun read(cbuf: CharArray, off: Int, len: Int) = throw IOException("I/O error")
        }

        try {
            UserWordStore.readImportText(boom, 1024)
            throw AssertionError("读失败必须冒出去")
        } catch (e: IOException) {
            assertEquals("I/O error", e.message)
        }
    }

    // ---- 已是 String 的那条路（剪贴板）----

    @Test
    fun stringOverloadAcceptsExactlyAtTheCap() {
        assertEquals("abcd", UserWordStore.readImportText("abcd", 4))
    }

    @Test
    fun stringOverloadRejectsOverTheCap() {
        // 剪贴板是别的应用能写的内容；超限必须返回 null 让调用方抛可见的拒绝
        assertNull(UserWordStore.readImportText("abcde", 4))
    }

    @Test
    fun stringOverloadEmptyIsEmptyNotRejection() {
        assertEquals("", UserWordStore.readImportText("", 4))
    }

    /**
     * 两条路径必须共用同一个上限 —— 否则「文件导入」与「剪贴板导入」会各有一套口径。
     * 调用方都靠**默认值**（不传 limit），所以这里也只验默认值：同一个超限串喂两条路。
     */
    @Test
    fun bothPathsDefaultToTheSameCap() {
        val over = "x".repeat(UserWordStore.MAX_IMPORT_CHARS + 1)
        assertNull("剪贴板路默认上限拦不住", UserWordStore.readImportText(over))
        assertNull("文件路默认上限拦不住", UserWordStore.readImportText(StringReader(over)))
    }

    @Test
    fun productionCapIsGenerousEnoughForTheEngineCeiling() {
        // 上限是 DoS 护栏、**不是数据规则**：必须显著高于引擎 MAX_IMPORT_WORDS = 100_000 条
        // 所能产生的 JSON（实测导出 `{"text":"…","freq":N}` 约 25 字符/条 ⇒ 最大约 2.5M 字符）。
        // 取小了就会把引擎本来收得下的合法词表挡在门外，而用户看到的文案是「文件过大」。
        assert(UserWordStore.MAX_IMPORT_CHARS >= 4 * 2_500_000) {
            "上限 ${UserWordStore.MAX_IMPORT_CHARS} 容不下引擎上限的词表"
        }
    }
}
