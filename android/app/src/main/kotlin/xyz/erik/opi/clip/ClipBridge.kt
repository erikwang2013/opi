// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import android.os.Handler
import android.os.HandlerThread
import xyz.erik.opi.jni.OpiEngine

/**
 * opi-clip 的 JNI 出口抽象（生产 = [OpiClipNative]，JVM 测试注入假实现）。
 * 方法名/签名与 `crates/opi-ffi/src/jni_clip.rs` 的注册表一一对应。
 */
internal interface ClipNativeApi {
    fun create(configJson: String?): Long
    fun destroy(handle: Long)
    fun start(handle: Long): Boolean
    fun stop(handle: Long)
    fun localText(handle: Long, text: String?): Int
    fun pollEvent(handle: Long, timeoutMs: Int): String?
    fun pairStart(handle: Long, peerId: String): Boolean
    fun pairConfirm(handle: Long, peerId: String, accept: Boolean): Boolean
    fun pairCancel(handle: Long, peerId: String): Boolean
    fun peers(handle: Long): String?
    fun forgetPeer(handle: Long, peerId: String): Boolean
    fun status(handle: Long): String?
    fun lastError(): String
}

/** 生产实现：只做转发；`System.loadLibrary` 在 [OpiEngine] 的 init 里。 */
internal object OpiClipNative : ClipNativeApi {
    override fun create(configJson: String?): Long = OpiEngine.clipCreate(configJson)
    override fun destroy(handle: Long) = OpiEngine.clipDestroy(handle)
    override fun start(handle: Long): Boolean = OpiEngine.clipStart(handle)
    override fun stop(handle: Long) = OpiEngine.clipStop(handle)
    override fun localText(handle: Long, text: String?): Int = OpiEngine.clipLocalText(handle, text)
    override fun pollEvent(handle: Long, timeoutMs: Int): String? =
        OpiEngine.clipPollEvent(handle, timeoutMs)
    override fun pairStart(handle: Long, peerId: String): Boolean =
        OpiEngine.clipPairStart(handle, peerId)
    override fun pairConfirm(handle: Long, peerId: String, accept: Boolean): Boolean =
        OpiEngine.clipPairConfirm(handle, peerId, accept)
    override fun pairCancel(handle: Long, peerId: String): Boolean =
        OpiEngine.clipPairCancel(handle, peerId)
    override fun peers(handle: Long): String? = OpiEngine.clipPeers(handle)
    override fun forgetPeer(handle: Long, peerId: String): Boolean =
        OpiEngine.clipForgetPeer(handle, peerId)
    override fun status(handle: Long): String? = OpiEngine.clipStatus(handle)
    override fun lastError(): String = OpiEngine.clipLastError()
}

/** 单线程调度（JVM 测试注入手动队列，生产 = [HandlerWorker]）。 */
internal interface ClipWorker {
    fun post(delayMs: Long, block: () -> Unit)
    fun quit()
}

/** 生产调度：一条专用 [HandlerThread]。所有 FFI 调用都必须落在它上面。 */
internal class HandlerWorker(name: String) : ClipWorker {
    private val thread = HandlerThread(name)
    private val handler: Handler

    init {
        // 不拦住进程退出；进程存活由 IME / 前台服务负责。
        thread.isDaemon = true
        thread.start()
        handler = Handler(thread.looper)
    }

    override fun post(delayMs: Long, block: () -> Unit) {
        handler.postDelayed(Runnable { block() }, delayMs)
    }

    override fun quit() {
        thread.quitSafely()
    }
}

/**
 * 内核句柄的宿主薄壳。**所有** FFI 调用（含 create/destroy）都在同一条
 * [ClipWorker] 线程上执行 —— Rust 侧句柄按创建线程校验，跨线程调用一律失败。
 *
 * `start` 之后在 worker 上循环 `pollEvent`（每次最多等 [pollWaitMs]），事件回调也在
 * worker 线程上；Android 层（[ClipRuntime]）负责把它们转回主线程。`pollEvent` 返回
 * null 是内核失败，空串是这段时间没有事件。
 */
internal class ClipBridge(
    private val api: ClipNativeApi,
    private val worker: ClipWorker,
    private val callback: Callback,
    private val pollWaitMs: Int = 250,
    private val refreshEveryPolls: Int = 8,
) {
    interface Callback {
        fun onStatus(status: ClipStatus)
        fun onPeers(peers: List<ClipPeer>)
        fun onPairingPrompt(prompt: ClipPairingPrompt)
        fun onPairingFinished(instanceId: String, trusted: Boolean, reason: String?)
        fun onRemoteText(instanceId: String, text: String)
        fun onError(message: String)
    }

    @Volatile private var started = false
    private var handle = 0L
    private var idlePolls = 0

    fun start(configJson: String) {
        if (started) return
        started = true
        worker.post(0) { open(configJson) }
    }

    fun publishLocalText(text: String) {
        if (!started) return
        worker.post(0) {
            if (!started || handle == 0L) return@post
            val code = runCatching { api.localText(handle, text) }
                .getOrElse { failFrom(it); -1 }
            // 0=已发布 1=防回环抑制（正常）；-1 已由内核记 last_error。
            if (code < 0) fail(lastError())
        }
    }

    fun pairStart(peerId: String) = call({ api.pairStart(handle, peerId) }, refresh = true)

    fun pairConfirm(peerId: String, accept: Boolean) =
        call({ api.pairConfirm(handle, peerId, accept) }, refresh = true)

    fun forgetPeer(peerId: String) = call({ api.forgetPeer(handle, peerId) }, refresh = true)

    fun stop() {
        if (!started && handle == 0L) return
        started = false
        worker.post(0) {
            closeHandle()
            worker.quit()
        }
    }

    private fun open(configJson: String) {
        if (!started) return
        val created = runCatching { api.create(configJson) }
            .getOrElse { failFrom(it); 0L }
        if (created == 0L) {
            fail(lastError())
            started = false
            return
        }
        handle = created
        if (!started) {
            closeHandle()
            return
        }
        val ok = runCatching { api.start(handle) }
            .getOrElse { failFrom(it); false }
        if (!ok) {
            fail(lastError())
            started = false
            closeHandle()
            return
        }
        publishStatus()
        publishPeers()
        poll()
    }

    private fun poll() {
        if (!started || handle == 0L) return
        val json = runCatching { api.pollEvent(handle, pollWaitMs) }
            .getOrElse { failFrom(it); null }
        if (json == null) {
            fail(lastError())
            started = false
            closeHandle()
            return
        }
        if (json.isEmpty()) {
            if (++idlePolls >= refreshEveryPolls) {
                idlePolls = 0
                publishPeers()
                publishStatus()
            }
        } else {
            idlePolls = 0
            handleEvent(json)
            publishPeers()
        }
        if (started && handle != 0L) worker.post(0) { poll() }
    }

    private fun handleEvent(json: String) {
        when (val event = parseClipEvent(json)) {
            is ClipEvent.PeerDiscovered,
            is ClipEvent.PeerOffline -> publishPeers()
            is ClipEvent.PairingPrompt -> callback.onPairingPrompt(
                ClipPairingPrompt(event.instanceId, event.sas, event.incoming),
            )
            is ClipEvent.PairingFinished -> {
                callback.onPairingFinished(event.instanceId, event.trusted, event.reason)
                publishPeers()
            }
            is ClipEvent.ClipboardReceived -> callback.onRemoteText(event.instanceId, event.text)
            is ClipEvent.SendFailed -> callback.onError("发送到一台已配对设备失败")
            is ClipEvent.Error -> callback.onError(event.message)
            is ClipEvent.Unknown, null -> Unit // 新事件类型：老宿主忽略，不崩
        }
    }

    private fun call(block: () -> Boolean, refresh: Boolean) {
        if (!started) return
        worker.post(0) {
            if (!started || handle == 0L) return@post
            val ok = runCatching(block).getOrElse { failFrom(it); false }
            if (!ok) fail(lastError())
            if (refresh) publishPeers()
        }
    }

    private fun publishStatus() {
        if (handle == 0L) return
        val json = runCatching { api.status(handle) }.getOrNull() ?: return
        val status = runCatching { parseClipStatus(json) }.getOrNull() ?: return
        callback.onStatus(status)
    }

    private fun publishPeers() {
        if (handle == 0L) return
        val json = runCatching { api.peers(handle) }.getOrNull() ?: return
        callback.onPeers(parseClipPeers(json))
    }

    private fun closeHandle() {
        if (handle == 0L) return
        runCatching { api.stop(handle) }
        runCatching { api.destroy(handle) }
        handle = 0L
    }

    private fun fail(message: String) {
        callback.onError(message.ifEmpty { "剪贴板内核调用失败" })
    }

    private fun failFrom(error: Throwable) {
        callback.onError(error.message ?: error.javaClass.simpleName)
    }

    private fun lastError(): String = runCatching { api.lastError() }.getOrDefault("")
}
