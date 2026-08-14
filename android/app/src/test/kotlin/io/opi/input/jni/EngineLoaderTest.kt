package io.opi.input.jni

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.IOException

/**
 * EngineLoader 纯 JVM 测试（A3/A4 模式：注入 FileOps/LoadApi 假实现，无 android.*）。
 * 对齐 flutter _loadLuna 规则：不存在或 size 不一致即重拷；I/O 失败或 load 失败均回退内置词库。
 */
class EngineLoaderTest {

    /** 假文件：asset 字节可控；existingSize 为 null 表示目标文件不存在。 */
    private class FakeFileOps(
        private val asset: ByteArray,
        existingSize: Long?,
    ) : EngineLoader.FileOps {
        private var size = existingSize
        var writeCalls = 0
        var written: ByteArray? = null

        override fun assetLength(): Long? = asset.size.toLong()

        override fun readAsset(): ByteArray = asset

        override fun existingSize(): Long? = size

        override fun write(bytes: ByteArray) {
            writeCalls++
            written = bytes
            size = bytes.size.toLong()
        }
    }

    /** 假引擎：记录 load 调用；ok=false 模拟坏路径（Rust load_or_fallback 语义）。 */
    private class FakeLoadApi(var ok: Boolean = true) : EngineLoader.LoadApi {
        val calls = mutableListOf<String?>()

        override fun load(path: String?): Boolean {
            calls += path
            return ok
        }
    }

    @Test
    fun missingFileTriggersCopyAndLoads() {
        val fileOps = FakeFileOps(byteArrayOf(1, 2, 3), existingSize = null)
        val api = FakeLoadApi()

        val ok = EngineLoader.loadAsset(fileOps, api, "/data/luna.opid")

        assertTrue(ok)
        assertEquals(1, fileOps.writeCalls)
        assertEquals(listOf("/data/luna.opid"), api.calls)
    }

    @Test
    fun sizeMismatchTriggersRecopy() {
        val fileOps = FakeFileOps(byteArrayOf(1, 2, 3), existingSize = 10L) // 旧文件残留
        val api = FakeLoadApi()

        EngineLoader.loadAsset(fileOps, api, "/data/luna.opid")

        assertEquals(1, fileOps.writeCalls)
        assertTrue(fileOps.written!!.contentEquals(byteArrayOf(1, 2, 3)))
    }

    @Test
    fun sameSizeSkipsCopy() {
        val fileOps = FakeFileOps(byteArrayOf(1, 2, 3), existingSize = 3L)
        val api = FakeLoadApi()

        EngineLoader.loadAsset(fileOps, api, "/data/luna.opid")

        assertEquals(0, fileOps.writeCalls)
        assertEquals(listOf("/data/luna.opid"), api.calls)
    }

    @Test
    fun badPathFallsBackToBuiltinDict() {
        // load(path) 返回 false（坏路径/损坏）→ 回退 load(null) 内置 35 词词库
        val fileOps = FakeFileOps(byteArrayOf(1, 2, 3), existingSize = null)
        val api = FakeLoadApi(ok = false)

        val ok = EngineLoader.loadAsset(fileOps, api, "/bad/luna.opid")

        assertFalse(ok)
        assertEquals(listOf("/bad/luna.opid", null), api.calls)
    }

    @Test
    fun needsCopyRule() {
        assertTrue(EngineLoader.needsCopy(100, null))      // 不存在
        assertTrue(EngineLoader.needsCopy(100, 99))        // size 不一致
        assertFalse(EngineLoader.needsCopy(100, 100))      // 一致 → 跳过
    }

    @Test
    fun assetReadFailureFallsBackToBuiltinDict() {
        // 资产读取抛 IOException → 不得崩溃；回退 load(null) 内置 35 词词库并返回 false
        val fileOps = object : EngineLoader.FileOps {
            override fun assetLength(): Long? = 100L
            override fun readAsset(): ByteArray = throw IOException("asset missing")
            override fun existingSize(): Long? = null
            override fun write(bytes: ByteArray) {}
        }
        val api = FakeLoadApi()

        val ok = EngineLoader.loadAsset(fileOps, api, "/data/luna.opid")

        assertFalse(ok)
        assertEquals(listOf(null), api.calls)
    }
}
