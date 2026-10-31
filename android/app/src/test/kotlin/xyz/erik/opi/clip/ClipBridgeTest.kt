// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 内核薄壳的开关/事件流（纯 JVM）：所有 FFI 调用串行在同一条 worker 上、
 * poll 事件分派、本地发布、配对确认、stop 的清理顺序、失败哨兵。
 *
 * [QueueWorker] 手动泵任务：poll 会无限自我重排，所以按次数 drain（不是清空）。
 */
class ClipBridgeTest {

    private class FakeApi : ClipNativeApi {
        var handle: Long = 42
        var createResult: Long = 42
        var createCalls = 0
        var startCalls = 0
        var stopCalls = 0
        var destroyCalls = 0
        var pollResult: String? = ""
        var lastErrorText = "内核错误"
        val localTexts = mutableListOf<String>()
        val confirms = mutableListOf<Boolean>()
        val forgotten = mutableListOf<String>()
        var peersJson = "[]"
        var statusJson =
            """{"device_name":"测试机","discovery_port":45888,"fingerprint":"0011223344556677","instance_id":"aa","peers":0,"running":true,"tcp_port":0}"""

        override fun create(configJson: String?): Long {
            createCalls++
            return createResult
        }

        override fun destroy(handle: Long) {
            destroyCalls++
        }

        override fun start(handle: Long): Boolean {
            startCalls++
            return true
        }

        override fun stop(handle: Long) {
            stopCalls++
        }

        override fun localText(handle: Long, text: String?): Int {
            localTexts += text ?: ""
            return 0
        }

        override fun pollEvent(handle: Long, timeoutMs: Int): String? = pollResult

        override fun pairStart(handle: Long, peerId: String): Boolean = true

        override fun pairConfirm(handle: Long, peerId: String, accept: Boolean): Boolean {
            confirms += accept
            return true
        }

        override fun pairCancel(handle: Long, peerId: String): Boolean = true

        override fun peers(handle: Long): String? = peersJson

        override fun forgetPeer(handle: Long, peerId: String): Boolean {
            forgotten += peerId
            return true
        }

        override fun status(handle: Long): String? = statusJson

        override fun lastError(): String = lastErrorText
    }

    private class QueueWorker : ClipWorker {
        private val queue = ArrayDeque<() -> Unit>()
        override fun post(delayMs: Long, block: () -> Unit) {
            queue.addLast(block)
        }

        override fun quit() = Unit

        /** 依次执行最多 [limit] 个任务（poll 循环会持续排队，不能清空）。 */
        fun drain(limit: Int = 20): Int {
            var done = 0
            while (done < limit) {
                val task = queue.removeFirstOrNull() ?: break
                task()
                done++
            }
            return done
        }
    }

    private class Recorder : ClipBridge.Callback {
        val statuses = mutableListOf<ClipStatus>()
        val peerLists = mutableListOf<List<ClipPeer>>()
        val prompts = mutableListOf<ClipPairingPrompt>()
        val finished = mutableListOf<Pair<String, Boolean>>()
        val remote = mutableListOf<Pair<String, String>>()
        val errors = mutableListOf<String>()

        override fun onStatus(status: ClipStatus) {
            statuses += status
        }

        override fun onPeers(peers: List<ClipPeer>) {
            peerLists += peers
        }

        override fun onPairingPrompt(prompt: ClipPairingPrompt) {
            prompts += prompt
        }

        override fun onPairingFinished(instanceId: String, trusted: Boolean, reason: String?) {
            finished += instanceId to trusted
        }

        override fun onRemoteText(instanceId: String, text: String) {
            remote += instanceId to text
        }

        override fun onError(message: String) {
            errors += message
        }
    }

    private fun bridge(
        api: FakeApi,
        worker: QueueWorker,
        recorder: Recorder,
    ): ClipBridge = ClipBridge(api, worker, recorder, pollWaitMs = 1, refreshEveryPolls = 8)

    @Test
    fun startCreatesAndStartsTheHandleOnTheWorkerThread() {
        val api = FakeApi()
        val worker = QueueWorker()
        val recorder = Recorder()
        val bridge = bridge(api, worker, recorder)
        bridge.start("""{"data_dir":"/tmp"}""")
        // start 只排队；真正的 create/start 发生在 drain 时。
        assertEquals(0, api.createCalls)
        worker.drain(1)
        assertEquals(1, api.createCalls)
        assertEquals(1, api.startCalls)
        assertEquals(1, recorder.statuses.size)
        assertTrue(recorder.statuses.single().running)
        assertEquals("测试机", recorder.statuses.single().deviceName)
        assertTrue(bridgeStop(bridge, worker, api))
    }

    @Test
    fun clipboardEventIsDeliveredVerbatim() {
        val api = FakeApi().apply {
            pollResult =
                """{"type":"clipboard_received","instance_id":"peer1","text":"remote \"x\"\n好","version":{"wall_ms":1}}"""
        }
        val worker = QueueWorker()
        val recorder = Recorder()
        val bridge = bridge(api, worker, recorder)
        bridge.start("{}")
        worker.drain(1)
        assertEquals(listOf("peer1" to "remote \"x\"\n好"), recorder.remote)
    }

    @Test
    fun pairingPromptAndFinishAreRoutedAndConfirmGoesToNative() {
        val api = FakeApi().apply {
            pollResult = """{"type":"pairing_prompt","incoming":false,"instance_id":"peer2","sas":"654321"}"""
        }
        val worker = QueueWorker()
        val recorder = Recorder()
        val bridge = bridge(api, worker, recorder)
        bridge.start("{}")
        worker.drain(1)
        assertEquals(ClipPairingPrompt("peer2", "654321", incoming = false), recorder.prompts.single())
        bridge.pairConfirm("peer2", accept = true)
        worker.drain(2)
        assertEquals(listOf(true), api.confirms)
        bridgeStop(bridge, worker, api)
    }

    @Test
    fun localTextIsPublishedThroughTheSameWorker() {
        val api = FakeApi()
        val worker = QueueWorker()
        val bridge = bridge(api, worker, Recorder())
        bridge.start("{}")
        worker.drain(1)
        bridge.publishLocalText("本地复制")
        worker.drain(2)
        assertEquals(listOf("本地复制"), api.localTexts)
        bridgeStop(bridge, worker, api)
    }

    @Test
    fun stopStopsAndDestroysThenQuits() {
        val api = FakeApi()
        val worker = QueueWorker()
        val bridge = bridge(api, worker, Recorder())
        bridge.start("{}")
        worker.drain(1)
        bridge.stop()
        worker.drain(2)
        assertEquals(1, api.stopCalls)
        assertEquals(1, api.destroyCalls)
        // stop 之后本地发布不再进内核。
        bridge.publishLocalText("after stop")
        worker.drain(1)
        assertTrue(api.localTexts.isEmpty())
    }

    @Test
    fun failedCreateReportsLastErrorAndDoesNotStart() {
        val api = FakeApi().apply { createResult = 0 }
        val worker = QueueWorker()
        val recorder = Recorder()
        val bridge = bridge(api, worker, recorder)
        bridge.start("{}")
        worker.drain(1)
        assertEquals(0, api.startCalls)
        assertEquals(listOf("内核错误"), recorder.errors)
    }

    @Test
    fun forgettingPeerRefreshesTheList() {
        val api = FakeApi()
        val worker = QueueWorker()
        val recorder = Recorder()
        val bridge = bridge(api, worker, recorder)
        bridge.start("{}")
        worker.drain(1)
        bridge.forgetPeer("peer3")
        worker.drain(2)
        assertEquals(listOf("peer3"), api.forgotten)
        assertFalse(recorder.peerLists.isEmpty())
    }

    private fun bridgeStop(bridge: ClipBridge, worker: QueueWorker, api: FakeApi): Boolean {
        bridge.stop()
        worker.drain(2)
        return api.destroyCalls == 1
    }
}
