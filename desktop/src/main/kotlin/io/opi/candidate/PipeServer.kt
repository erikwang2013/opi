// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! named pipe 服务器：接受 TSF 客户端连接 → 读 NDJSON 行更新 CandidateModel →
//! 回复 select/翻页（线协议全貌见 Main.kt 头部注释，与
//! crates/tsf-opi/src/candidate_io.rs 互为镜像，改协议须同步两处）。

package io.opi.candidate

import com.sun.jna.Function
import com.sun.jna.WString
import com.sun.jna.platform.win32.Kernel32
import com.sun.jna.platform.win32.WinBase
import com.sun.jna.platform.win32.WinNT
import com.sun.jna.ptr.IntByReference
import java.io.ByteArrayOutputStream
import kotlin.concurrent.thread

// ---------- 管道常量 ----------

/** 管道名（与 Rust 侧 candidate_io.rs 的 PIPE_NAME 一致）。 */
private const val PIPE_NAME = "\\\\.\\pipe\\opi-candidates"
private const val PIPE_ACCESS_DUPLEX = 0x3
/** CreateNamedPipe 的 dwOpenMode 标志：首个实例独占管道名，防恶意本地进程抢注。 */
private const val FILE_FLAG_FIRST_PIPE_INSTANCE = 0x80000
private const val PIPE_TYPE_BYTE = 0x0 // 字节流模式 + PIPE_WAIT（阻塞）
private const val PIPE_READMODE_BYTE = 0x0
private const val PIPE_WAIT = 0x0
private const val PIPE_BUF = 4096
private const val MAX_INSTANCES = 1

// ---------- named pipe 服务器（JNA kernel32，本窗为 SERVER） ----------

/** 管道服务器：接受 TSF 客户端连接 → 读消息更新模型 → 回复 select/翻页。 */
class PipeServer(private val model: CandidateModel) {

    /** 最近一次客户端的连接句柄（UI 点击 select/翻页用）。 */
    @Volatile
    var lastClientPipe: WinNT.HANDLE? = null
        private set

    fun start() {
        thread(isDaemon = true, name = "opi-candidates-pipe") { serveLoop() }
    }

    private fun serveLoop() {
        while (true) {
            val pipe = createPipe()
            if (pipe == null) {
                Thread.sleep(500) // 管道创建失败（命名冲突/权限）：退避重试
                continue
            }
            connect(pipe) // 阻塞至 TSF 进程连接
            lastClientPipe = pipe
            readLoop(pipe)
            // 读循环退出 = 客户端消失（TSF 宿主崩溃/被任务管理器结束/用户注销），
            // 此时收不到 hide 消息。必须自行隐藏：窗口是置顶+无边框+不可聚焦且无
            // 关闭入口，漏掉这一步就是一扇关不掉的窗，只能杀进程。
            model.visible = false
            lastClientPipe = null
            Kernel32.INSTANCE.DisconnectNamedPipe(pipe)
            Kernel32.INSTANCE.CloseHandle(pipe)
        }
    }

    private fun createPipe(): WinNT.HANDLE? {
        // jna-platform 的 Kernel32 接口无 CreateNamedPipeW → raw Function。
        val fn = Function.getFunction("kernel32", "CreateNamedPipeW")
        val h = fn.invoke(
            WinNT.HANDLE::class.java,
            arrayOf(
                WString(PIPE_NAME), PIPE_ACCESS_DUPLEX or FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE or PIPE_READMODE_BYTE or PIPE_WAIT,
                MAX_INSTANCES, PIPE_BUF, PIPE_BUF, 0, null,
            ),
        ) as WinNT.HANDLE
        return if (h == WinBase.INVALID_HANDLE_VALUE) null else h
    }

    private fun connect(pipe: WinNT.HANDLE) {
        // 阻塞式连接：非 0 = 成功；0 但 ERROR_PIPE_CONNECTED（535）= 已连接，均可继续。
        Function.getFunction("kernel32", "ConnectNamedPipe").invokeInt(arrayOf(pipe, null))
    }

    private fun readLoop(pipe: WinNT.HANDLE) {
        val buf = ByteArray(PIPE_BUF)
        // 残留必须是**字节**，不能是已解码的字符串：管道是字节流，ReadFile 不保证
        // 落在 UTF-8 字符边界上。先解码再分帧的话，被切开的多字节字符会在这一侧
        // 永久变成 U+FFFD（Java 对非法序列静默替换），后半个字符补上来也救不回 ——
        // 候选窗显示乱码。Rust 侧 candidate_io.rs 的 pending 就是 Vec<u8>，
        // 两处「互为镜像」的约定在这里必须同构。
        val pending = ByteArrayOutputStream()
        while (true) {
            val n = IntByReference()
            // 字节流阻塞读：失败或 0 字节 = 客户端断开。
            if (!Kernel32.INSTANCE.ReadFile(pipe, buf, buf.size, n, null) || n.value <= 0) return
            pending.write(buf, 0, n.value)
            drainLines(pending)
        }
    }

    /**
     * 按 '\n' 切出完整行并解码，残字节留在 [pending] 等下一次 ReadFile 补齐。
     * 每次全量 `toByteArray()` 是 O(已有字节)：候选窗消息很短且读取不频繁，
     * 不值得为此引入环形缓冲。
     */
    private fun drainLines(pending: ByteArrayOutputStream) {
        val bytes = pending.toByteArray()
        var start = 0
        for (i in bytes.indices) {
            if (bytes[i] == '\n'.code.toByte()) {
                handleLine(String(bytes, start, i - start, Charsets.UTF_8))
                start = i + 1
            }
        }
        pending.reset()
        if (start < bytes.size) pending.write(bytes, start, bytes.size - start)
    }

    private fun handleLine(line: String) {
        try {
            val obj = parseLine(line) ?: return
            when ((obj["type"] as? JVal.JStr)?.v) {
                "show" -> {
                    model.buffer = (obj["buffer"] as? JVal.JStr)?.v ?: ""
                    model.candidates = ((obj["candidates"] as? JVal.JArr)?.v
                        ?: emptyList()).mapNotNull { (it as? JVal.JStr)?.v }
                    model.page = ((obj["page"] as? JVal.JNum)?.v ?: 1L).toInt().coerceAtLeast(1)
                    model.pageCount = ((obj["page_count"] as? JVal.JNum)?.v ?: 1L).toInt().coerceAtLeast(1)
                    model.mode = (obj["mode"] as? JVal.JStr)?.v ?: "pinyin"
                    model.visible = true
                }
                "hide" -> model.visible = false
                "position" -> {
                    (obj["x"] as? JVal.JNum)?.let { model.x = it.v.toInt() }
                    (obj["y"] as? JVal.JNum)?.let { model.y = it.v.toInt() }
                }
            }
        } catch (e: Exception) {
            // 防御性兜底：单行解析/处理异常只丢弃该行，绝不杀死管道服务器线程。
        }
    }

    /** 点击候选 → TSF 提交（index 页内 0 起，与 TSF 侧 logic.select 一致）。 */
    fun sendSelect(index: Int) = send("""{"type":"select","index":$index}""")
    fun sendNextPage() = send("""{"type":"next_page"}""")
    fun sendPrevPage() = send("""{"type":"prev_page"}""")

    /** partial-write 循环（镜像 Rust 侧 candidate_io.rs）：写满或失败为止；失败返回 false。 */
    private fun send(json: String): Boolean {
        val pipe = lastClientPipe ?: return false
        val bytes = (json + "\n").toByteArray(Charsets.UTF_8)
        var off = 0
        while (off < bytes.size) {
            val written = IntByReference()
            // JNA byte[] 无偏移参数 → 切片传入剩余部分；实际写入字节数由 out 参数返回。
            val ok = Kernel32.INSTANCE.WriteFile(
                pipe, bytes.copyOfRange(off, bytes.size), bytes.size - off, written, null,
            )
            if (!ok) return false
            if (written.value <= 0) return false // 0 字节写入：对端异常，防死循环
            off += written.value
        }
        return true
    }
}
