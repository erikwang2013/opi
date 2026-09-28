// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! NDJSON 解析器的嵌套深度上限（Protocol.kt）。纯 JVM，无 JNA、无 Windows 依赖
//! —— 这是 desktop/ 唯一能在本机真跑的部分（管道与 kernel32 只能在 Windows 验收）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class ProtocolDepthTest {

    /**
     * 一行**合法**消息，`"a"` 的值是 [depth] 层嵌套数组：`{"a":[[[…]]]}`。
     * 容器总数 = depth + 1（外层对象），界限见 [depthBoundaryMatchesSerdeJson]。
     * （注意不能直接测顶层数组：`parseLine` 只接受对象，任何深度的 `[]` 都返回
     * null —— 那样测出来的红/绿都是假的。）
     */
    private fun nestedLine(depth: Int) = "{\"a\":" + "[".repeat(depth) + "]".repeat(depth) + "}"

    /**
     * 上万层 `[[[[…` 不许把线程栈吃光：递归下降在深度上限处返回 null（该行丢弃）。
     * 修好之前这里是 StackOverflowError（本机 JVM 实测约 2k–5k 层即溢出）—— 它是
     * Error 不是 Exception，会穿出 readLoop/serveLoop，管道线程死亡且不再重建
     * （症状只有「候选窗再也不出现」）。
     */
    @Test
    fun deepNestingIsRejectedInsteadOfOverflowing() {
        assertNull(parseLine(nestedLine(100_000)))
    }

    /** 只有开括号、没有闭括号的同类输入同样不许溢出。 */
    @Test
    fun unterminatedDeepNestingIsRejected() {
        assertNull(parseLine("{\"a\":" + "[".repeat(1 shl 20)))
    }

    /**
     * 边界镜像 serde_json 的 `remaining_depth: 128`（check_recursion! 先减再判 0）：
     * 最多 127 层容器可解析，第 128 层起该行丢弃。
     */
    @Test
    fun depthBoundaryMatchesSerdeJson() {
        assertNotNull("127 层容器应可解析", parseLine(nestedLine(126)))
        assertNull("128 层容器应被拒绝", parseLine(nestedLine(127)))
    }

    /** 正常 show 消息仍必须解析（深度门禁不能误伤真实流量）。 */
    @Test
    fun realShowMessageParses() {
        val line =
            "{\"type\":\"show\",\"buffer\":\"ni\",\"candidates\":[\"你\",\"尼\"]," +
                "\"page\":2,\"page_count\":3,\"mode\":\"pinyin\"}"
        val obj = parseLine(line)
        assertNotNull(obj)
        val o = obj!!
        assertEquals("show", (o["type"] as JVal.JStr).v)
        assertEquals("ni", (o["buffer"] as JVal.JStr).v)
        assertEquals(2, (o["candidates"] as JVal.JArr).v.size)
        assertEquals(3L, (o["page_count"] as JVal.JNum).v)
    }

    /**
     * 兄弟节点共享深度预算：一个深容器不该让下一个容器误判 —— 即 depth 必须在
     * 退出容器时归还。不归还的实现（只增不减）会让第二个分支从 121 层起算并越界。
     */
    @Test
    fun depthIsRestoredBetweenSiblings() {
        val deep = "[".repeat(120) + "]".repeat(120)
        assertNotNull(parseLine("{\"a\":$deep,\"b\":$deep}"))
    }
}
