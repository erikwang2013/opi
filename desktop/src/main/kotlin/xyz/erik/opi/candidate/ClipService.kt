// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 跨设备剪贴板的**宿主**：`--statusbar` 常驻进程里的一条专用线程独占 FFI 句柄。
//!
//! 为什么单独一条线程：`opi_clip_*` 的句柄是 thread-confined（见 cabi_clip.rs）。
//! create/start/poll/publish/destroy **全在这里**；设置窗只能通过 [ClipControlServer]
//! 入队命令，绝不直接碰句柄。
//!
//! 方向：本机剪贴板变化 → `opi_clip_local_text` 广播；远端事件 → 写回本机剪贴板。
//! 不自动粘贴、不记录内容、不落盘。防回环由内核负责，适配器再挡一次自己写入的回声。

package xyz.erik.opi.candidate

import com.sun.jna.Pointer
import java.nio.file.Files
import java.security.SecureRandom
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

private const val PEER_REFRESH_MS = 1000L
private const val EVENT_POLL_MS = 100
private const val IDLE_POLL_MS = 200L
private const val LOCAL_POLL_MS = 300L
private const val CORE_RETRY_NS = 2_000_000_000L

/** 入口：状态栏组合时调用（幂等）。 */
internal object ClipHost {
    private var service: ClipService? = null

    @Synchronized
    fun ensureStarted(): ClipService {
        service?.let { return it }
        val s = ClipService()
        service = s
        s.start()
        return s
    }

    fun setEnabled(on: Boolean) {
        ensureStarted().setEnabled(on)
    }
}

internal class ClipService {
    private val windows = isWindows()
    private val mode = currentClipMode(windows)
    private val deviceName = clipDeviceName(
        System.getenv("COMPUTERNAME") ?: System.getenv("HOSTNAME"),
    )
    private val commands = LinkedBlockingQueue<ClipCommand>()
    private val token = randomToken()

    @Volatile private var enabled = false
    @Volatile private var closed = false
    @Volatile private var snapshot: String = ""
    private var thread: Thread? = null
    private var control: ClipControlServer? = null

    // ---- 以下字段只在剪贴板线程上读写 ----
    private var ffi: OpiClipFfi? = null
    private var ffiLoadAttempted = false
    private var ffiError: String? = null
    private var handle: Pointer? = null
    private var adapter: ClipboardAdapter? = null
    private var peers: List<ClipPeer> = emptyList()
    private var ui = ClipUiState()
    private var lastPeersAt = 0L
    private var lastLocalAt = 0L
    private var lastCoreAttempt = 0L

    init {
        publishSnapshot()
    }

    fun start() {
        thread = Thread({ run() }, "opi-clip").apply {
            isDaemon = true
            start()
        }
        control = ClipControlServer(
            controlFile = clipControlPath(),
            token = token,
            status = { snapshot },
            enqueue = { commands.offer(it) },
        ).also { it.start() }
        runCatching {
            Runtime.getRuntime().addShutdownHook(Thread({ close() }, "opi-clip-stop"))
        }
    }

    fun setEnabled(on: Boolean) {
        if (!closed) commands.offer(ClipCommand.SetEnabled(on))
    }

    fun close() {
        if (closed) return
        closed = true
        runCatching { control?.close() }
        runCatching { thread?.join(1500) }
    }

    // ---------- 循环（剪贴板线程） ----------

    private fun run() {
        try {
            while (!closed) {
                if (commands.poll(if (enabled) 0L else IDLE_POLL_MS, TimeUnit.MILLISECONDS) != null) {
                    while (true) handleCommand(commands.poll() ?: break)
                }
                if (closed) break
                if (!enabled) continue
                ensureCore()
                val f = ffi ?: run {
                    Thread.sleep(IDLE_POLL_MS)
                    continue
                }
                val h = handle ?: run {
                    Thread.sleep(IDLE_POLL_MS)
                    continue
                }
                pollLocal()
                pollEvents(f, h)
                maybeRefreshPeers(f, h)
            }
        } finally {
            stopCore()
        }
    }

    private fun handleCommand(cmd: ClipCommand) {
        when (cmd) {
            is ClipCommand.SetEnabled -> setEnabledState(cmd.on)
            is ClipCommand.Pair -> withPeer(cmd.peer) { f, h, arg ->
                f.opi_clip_pair_start(h, arg.memory, arg.len)
            }
            is ClipCommand.Confirm -> withPeer(cmd.peer) { f, h, arg ->
                f.opi_clip_pair_confirm(
                    h, arg.memory, arg.len, if (cmd.accept) 1.toByte() else 0.toByte(),
                )
            }
            is ClipCommand.Forget -> {
                withPeer(cmd.peer) { f, h, arg ->
                    f.opi_clip_forget_peer(h, arg.memory, arg.len)
                }
                refreshPeersNow()
            }
            is ClipCommand.SendText -> {
                if (!enabled) return
                if (cmd.text.length > CLIP_TEXT_MAX_CHARS) {
                    ui = ui.copy(lastError = "文本超过 256 KiB 上限，未发送")
                    publishSnapshot()
                } else {
                    publish(cmd.text)
                }
            }
        }
    }

    private fun setEnabledState(on: Boolean) {
        if (on) {
            if (enabled && handle != null) return
            enabled = true
            ffiLoadAttempted = false
            lastCoreAttempt = 0L
            ui = ui.copy(lastError = null)
            publishSnapshot()
        } else {
            if (!enabled && handle == null) return
            stopCore()
            peers = emptyList()
            ui = ClipUiState()
            publishSnapshot()
        }
    }

    /** 在剪贴板线程上执行一条 peer 命令；peer 参数缓冲活到调用结束。 */
    private inline fun withPeer(peer: String, call: (OpiClipFfi, Pointer, Utf16Arg) -> Byte) {
        val f = ffi
        val h = handle
        if (f == null || h == null) {
            ui = ui.copy(lastError = "剪贴板内核未启动")
            publishSnapshot()
            return
        }
        val arg = Utf16Arg(peer)
        try {
            if (call(f, h, arg) == 0.toByte()) {
                ui = ui.copy(lastError = readLastError(f))
            } else {
                ui = ui.copy(lastError = null)
            }
        } catch (t: Throwable) {
            ui = ui.copy(lastError = transportError(t))
        }
        publishSnapshot()
    }

    // ---------- 内核生命周期 ----------

    private fun ensureCore() {
        if (handle != null) return
        val now = System.nanoTime()
        if (lastCoreAttempt != 0L && now - lastCoreAttempt < CORE_RETRY_NS) return
        lastCoreAttempt = now

        val f = ffi ?: run {
            if (ffiLoadAttempted) return
            ffiLoadAttempted = true
            when (val loaded = loadClipFfi()) {
                is FfiLoad.Loaded -> {
                    ffi = loaded.ffi
                    ffiError = null
                    loaded.ffi
                }
                is FfiLoad.Unavailable -> {
                    ffiError = loaded.reason
                    publishSnapshot()
                    null
                }
            }
        } ?: return

        try {
            Files.createDirectories(clipDataDir())
        } catch (t: Throwable) {
            ui = ui.copy(lastError = "无法创建数据目录：${t.javaClass.simpleName}")
            publishSnapshot()
            return
        }
        val config = Utf16Arg(clipConfigJson(clipDataDir().toString(), deviceName))
        val clip = f.opi_clip_create(config.memory, config.len)
        if (clip == null) {
            ui = ui.copy(lastError = readLastError(f))
            publishSnapshot()
            return
        }
        if (f.opi_clip_start(clip) == 0.toByte()) {
            val why = readLastError(f)
            f.opi_clip_destroy(clip)
            ui = ui.copy(lastError = why)
            publishSnapshot()
            return
        }
        handle = clip
        adapter = createClipboardAdapter(windows, mode)
        if (adapter == null) {
            ui = ui.copy(lastError = "剪贴板适配器不可用（当前桌面环境不支持）")
        } else {
            ui = ui.copy(lastError = null)
        }
        peers = emptyList()
        lastPeersAt = 0L
        lastLocalAt = 0L
        publishSnapshot()
    }

    private fun stopCore() {
        val f = ffi
        val h = handle
        if (f != null && h != null) {
            runCatching { f.opi_clip_stop(h) }
            runCatching { f.opi_clip_destroy(h) }
        }
        handle = null
        runCatching { adapter?.close() }
        adapter = null
        enabled = false
        publishSnapshot()
    }

    // ---------- 本机 → 远端 ----------

    private fun pollLocal() {
        val now = System.nanoTime()
        if ((now - lastLocalAt) / 1_000_000L < LOCAL_POLL_MS) return
        lastLocalAt = now
        val text = try {
            adapter?.poll()
        } catch (t: Throwable) {
            ui = ui.copy(lastError = transportError(t))
            publishSnapshot()
            null
        } ?: return
        if (text.length > CLIP_TEXT_MAX_CHARS) {
            ui = ui.copy(lastError = "本机剪贴板文本超过 256 KiB，未发送")
            publishSnapshot()
            return
        }
        publish(text)
    }

    private fun publish(text: String) {
        val f = ffi
        val h = handle
        if (f == null || h == null) {
            ui = ui.copy(lastError = "剪贴板内核未启动，未发送")
            publishSnapshot()
            return
        }
        val arg = Utf16Arg(text)
        val rc = try {
            f.opi_clip_local_text(h, arg.memory, arg.len)
        } catch (t: Throwable) {
            ui = ui.copy(lastError = transportError(t))
            publishSnapshot()
            return
        }
        // 0 = 已发布，1 = 防回环抑制（正常），-1 = 失败。
        ui = ui.copy(lastError = if (rc < 0) readLastError(f) else null)
        publishSnapshot()
    }

    // ---------- 远端 → 本机 ----------

    private fun pollEvents(f: OpiClipFfi, h: Pointer) {
        val json = try {
            f.opi_clip_poll_event(h, EVENT_POLL_MS).takeText(f)
        } catch (t: Throwable) {
            ui = ui.copy(lastError = transportError(t))
            publishSnapshot()
            return
        }
        if (json.isEmpty()) return
        val event = parseClipEvent(json) ?: return
        if (event is ClipEvent.ClipboardReceived) {
            val next = ui.applyEvent(event)
            if (next === ui) return // 同一 event_id 重复投递：不重复写、不重复提示
            ui = next
            val written = adapter?.write(event.text) ?: false
            if (!written) {
                ui = ui.copy(lastError = "收到远端内容，但写入本机剪贴板失败")
            }
        } else {
            val pendingBefore = ui.pending.size
            ui = ui.applyEvent(event)
            if (ui.pending.size != pendingBefore || event is ClipEvent.PairingFinished) {
                refreshPeersNow()
            }
        }
        publishSnapshot()
    }

    private fun maybeRefreshPeers(f: OpiClipFfi, h: Pointer) {
        val now = System.nanoTime()
        if ((now - lastPeersAt) / 1_000_000L < PEER_REFRESH_MS) return
        lastPeersAt = now
        val json = runCatching { f.opi_clip_peers(h).takeText(f) }.getOrNull() ?: return
        applyPeers(parseClipPeers(json))
    }

    private fun refreshPeersNow() {
        val f = ffi ?: return
        val h = handle ?: return
        val json = runCatching { f.opi_clip_peers(h).takeText(f) }.getOrNull() ?: return
        applyPeers(parseClipPeers(json))
        lastPeersAt = System.nanoTime()
    }

    private fun applyPeers(list: List<ClipPeer>) {
        peers = list.sortedWith(compareBy({ !it.connected }, { !it.trusted }, { it.name }))
        // 事件里的名字可能比列表早到（pairing_prompt）；并集保证来源提示尽量给人名。
        ui = ui.copy(names = ui.names + peers.associate { it.instanceId to it.name })
        publishSnapshot()
    }

    // ---------- 杂项 ----------

    private fun readLastError(f: OpiClipFfi): String {
        val message = runCatching { f.opi_clip_last_error().takeText(f) }.getOrNull().orEmpty()
        return message.takeIf { it.isNotBlank() } ?: "剪贴板内核调用失败"
    }

    /** 异常文本可能带路径/端口，但不会带剪贴板内容；只留类型 + 短消息。 */
    private fun transportError(t: Throwable): String =
        "剪贴板内核异常：${t.javaClass.simpleName}" +
            (t.message?.take(120)?.let { "：$it" } ?: "")

    private fun buildStatus(): ClipStatus = ClipStatus(
        enabled = enabled,
        running = handle != null,
        mode = mode,
        deviceName = deviceName,
        peers = peers,
        pending = ui.pending.values.sortedBy { it.instanceId },
        lastSource = ui.lastSource,
        lastError = ui.lastError,
        ffiError = ffiError,
    )

    private fun publishSnapshot() {
        snapshot = encodeClipStatus(buildStatus())
    }

    private fun randomToken(): String {
        val bytes = ByteArray(16)
        SecureRandom().nextBytes(bytes)
        return buildString(32) {
            for (b in bytes) append((b.toInt() and 0xFF).toString(16).padStart(2, '0'))
        }
    }
}
