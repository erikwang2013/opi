// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! named pipe 服务器：接受 TSF 客户端连接 → 读 NDJSON 行更新 CandidateModel →
//! 回复 select/翻页（线协议全貌见 Main.kt 头部注释，与
//! crates/tsf-opi/src/candidate_io.rs 互为镜像，改协议须同步两处）。
//!
//! 【镜像清单（对端为不可信输入，两侧同构；Rust 侧见 candidate_io.rs 对应行）】
//!  1. 解析/回调抛任何 Throwable 都不许杀死管道线程：Rust 用 `catch_unwind`
//!     兜 `dispatch_line`，本侧 `catch (Throwable)` 兜 handleLine + serveLoop。
//!  2. 分帧残留有上限（[MAX_PENDING] ↔ Rust `MAX_PENDING`）：对端发一条不含 '\n'
//!     的超长消息，残留就会一直涨 —— 涨的是本进程内存。
//!  3. 句柄「取用」与「关闭」在同一把锁下，且关闭前确认仍是自身句柄
//!     （[connLock] ↔ Rust `Mutex<Option<PipeHandle>>` + `disconnect`/`Drop`）。
//!
//! 注意：单实例管道挡不住**同机**任意进程连接（本侧与前端的信任边界是「用户会话」，
//! 不是进程）；PIPE_REJECT_REMOTE_CLIENTS 只是纵深防御，真正的 DoS 防线是上面第 1、2 条。

package io.opi.candidate

import com.sun.jna.Native
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
/**
 * CreateNamedPipe 的 dwOpenMode 标志。**只约束本进程**：管道名已被别人占用时，
 * 本次 CreateNamedPipe 直接失败（ERROR_ACCESS_DENIED）→ 本侧退避重试。
 * 它**防不了**别人抢先创建 —— 反过来，是别人能挡住我们。
 */
private const val FILE_FLAG_FIRST_PIPE_INSTANCE = 0x80000
private const val PIPE_TYPE_BYTE = 0x0 // 字节流模式 + PIPE_WAIT（阻塞）
private const val PIPE_READMODE_BYTE = 0x0
private const val PIPE_WAIT = 0x0
/** 拒绝来自远程（SMB）的客户端 —— 纵深防御，见文件头「注意」段。 */
private const val PIPE_REJECT_REMOTE_CLIENTS = 0x00000008
private const val PIPE_BUF = 4096
private const val MAX_INSTANCES = 1

/** ConnectNamedPipe 返回 0 时的「客户端已抢先连上」错误码 —— 属正常，不是故障。 */
private const val ERROR_PIPE_CONNECTED = 535

/**
 * 分帧残留（pending）上限，语义与 Rust 侧 candidate_io.rs 的 `MAX_PENDING` 一致
 * （同为 1 MiB，理由同）：对端只要发一条不含 '\n' 的超长消息，残留就会一直涨 ——
 * 涨的是本进程的内存，等于把内存交给对端。1 MiB 远超单条消息（一页候选 JSON 至多
 * 几 KB）；超限即协议错乱/恶意对端 → 丢弃残留并退出 readLoop（serveLoop 随即隐藏
 * + 关句柄 + 重建）。
 */
private const val MAX_PENDING = 1 shl 20

// ---------- named pipe 服务器（JNA kernel32，本窗为 SERVER） ----------

/** 管道服务器：接受 TSF 客户端连接 → 读消息更新模型 → 回复 select/翻页。 */
class PipeServer(private val model: CandidateModel) {

    /**
     * 最近一次客户端的连接句柄（UI 点击 select/翻页用）。读写全部在 [connLock] 下 ——
     * 不用 `@Volatile`：裸 `@Volatile` 与 CloseHandle/CreateNamedPipeW 之间仍有窗口，
     * 见 [send] 与 [serveLoop] 的注释。
     */
    private var lastClientPipe: WinNT.HANDLE? = null

    /** 保护 [lastClientPipe]：句柄取用 + WriteFile 与「置 null + Disconnect + Close」互斥。 */
    private val connLock = Any()

    fun start() {
        thread(isDaemon = true, name = "opi-candidates-pipe") { serveLoop() }
    }

    private fun serveLoop() {
        while (true) {
            // pipe 保持可空只为 finally 判「本周期是否建出了句柄」；created 是实际句柄。
            var pipe: WinNT.HANDLE? = null
            try {
                val created = createPipe()
                if (created == null) {
                    Thread.sleep(500) // 管道创建失败（命名冲突/权限）：退避重试
                    continue
                }
                pipe = created
                if (!connect(created)) continue // 没连上：finally 关句柄，下一轮重建
                synchronized(connLock) { lastClientPipe = created }
                readLoop(created)
            } catch (t: Throwable) {
                // 任何 Throwable（含 Error：StackOverflowError / OutOfMemoryError）都不许
                // 杀死本线程 —— 线程一死 serveLoop 就再也不会重建管道，而表面症状只有
                // 「候选窗再也不出现」，零日志，是最难查的一类故障。镜像 Rust 侧读线程
                // 对 dispatch_line 的 catch_unwind（回调/解析都是外部代码，必须兜住）。
                if (t is InterruptedException) {
                    Thread.currentThread().interrupt()
                    return // 中断只在关闭路径上出现（Thread.sleep），如实退出而非忙转
                }
            } finally {
                // 读循环退出 = 客户端消失（TSF 宿主崩溃/被任务管理器结束/用户注销），
                // 此时收不到 hide 消息。必须自行隐藏：窗口是置顶+无边框+不可聚焦且无
                // 关闭入口，漏掉这一步就是一扇关不掉的窗，只能杀进程。
                // 放 finally = 异常路径同样执行「隐藏 + 关句柄 + 重建」，不留残窗口。
                model.visible = false
                val h = pipe
                if (h != null) {
                    // 取句柄/置空/Disconnect/Close 全在同一把锁下：CloseHandle 之后
                    // CreateNamedPipeW 很可能拿回同一个句柄号，若与 send() 交错，用户点
                    // 翻页那一瞬间就 WriteFile 进下一条连接。镜像 Rust 侧 conn 的
                    // Mutex<Option<PipeHandle>> + disconnect() 的「仅当仍是自身句柄才关」。
                    synchronized(connLock) {
                        if (lastClientPipe == h) lastClientPipe = null
                        Kernel32.INSTANCE.DisconnectNamedPipe(h)
                        Kernel32.INSTANCE.CloseHandle(h)
                    }
                }
            }
        }
    }

    private fun createPipe(): WinNT.HANDLE? {
        // 走 Kernel32.INSTANCE（以 W32APIOptions 装载）而非 raw Function.getFunction：
        // 函数名消歧义（CreateNamedPipe → CreateNamedPipeW，不存在时回落原名）由
        // W32APIFunctionMapper 负责，且只有这条路径之后 Native.getLastError() 才拿得到
        // 本次调用的错误码（见 connect）。
        val h = Kernel32.INSTANCE.CreateNamedPipe(
            PIPE_NAME,
            PIPE_ACCESS_DUPLEX or FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE or PIPE_READMODE_BYTE or PIPE_WAIT or PIPE_REJECT_REMOTE_CLIENTS,
            MAX_INSTANCES, PIPE_BUF, PIPE_BUF, 0, null,
        )
        return if (h == null || h == WinBase.INVALID_HANDLE_VALUE) null else h
    }

    /**
     * 阻塞式连接（PIPE_WAIT）。返回 false = 句柄不可用，由 serveLoop 的 finally 关掉、
     * 下一轮重建。
     *
     * ConnectNamedPipe 返回 0 有两个来源：真失败，或 ERROR_PIPE_CONNECTED(535) ——
     * 客户端在 CreateNamedPipeW 与本次调用之间抢先连上。后者是正常竞态不是故障
     * （Rust 侧客户端 50ms 退避重试连接，这个窗口真实存在）。区分二者只能靠
     * GetLastError；`err == 0` 视为「读不到错误码」，按正常放行 —— 真没连上时
     * readLoop 的首次 ReadFile 会立刻失败返回 → 自愈；反过来把「读不到」当失败，
     * 一旦 last-error 不可用就会**每次都杀掉正常连接**，比不判更坏。
     *
     * ⚠ 本机无 Windows，这条路径（含 Native.getLastError 在 raw/typed 混用下是否
     * 可靠）**未实测**，只经 javap 核对 jna-platform 5.6.0 确有这两个 typed 导出。
     */
    private fun connect(pipe: WinNT.HANDLE): Boolean {
        if (Kernel32.INSTANCE.ConnectNamedPipe(pipe, null)) return true
        val err = Native.getLastError()
        return err == ERROR_PIPE_CONNECTED || err == 0
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
            // 攒够上限还没有 '\n'：丢弃残留并退出 readLoop。只清残留不退出的话，
            // 对端接着发就接着涨 —— 等于把内存交给对端（语义同 Rust 侧 MAX_PENDING）。
            if (pending.size() > MAX_PENDING) return
            drainLines(pending)
        }
    }

    /**
     * 按 '\n' 切出完整行并解码，残字节留在 [pending] 等下一次 ReadFile 补齐。
     * 每次全量 `toByteArray()` 是 O(已有字节)：候选窗消息很短且读取不频繁，不值得
     * 为此引入环形缓冲。上界由 [MAX_PENDING] 兜住（1 MiB 内最坏 ~256 次全量拷贝，
     * 超限即断连），不存在「对端慢慢喂、本侧平方级烧 CPU」的通道。
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
                    // x/y 单位 = AWT 逻辑像素（约定见 Main.kt 头注释；勿按设备像素或
                    // density 缩放）。
                    (obj["x"] as? JVal.JNum)?.let { model.x = it.v.toInt() }
                    (obj["y"] as? JVal.JNum)?.let { model.y = it.v.toInt() }
                }
            }
        } catch (e: Throwable) {
            // 防御性兜底：单行解析/处理异常只丢弃该行，绝不杀死管道服务器线程。
            // 必须是 Throwable 而非 Exception：递归下降的 StackOverflowError（上万层
            // `[[[[…`）与 OOM 都是 Error，catch(Exception) 拦不住 —— 漏出去线程就死了，
            // 而 serveLoop 也再不会重建管道。（解析器现在另有 128 层上限兜底，见
            // Protocol.kt 的 MAX_DEPTH。）
        }
    }

    /** 点击候选 → TSF 提交（index 页内 0 起，与 TSF 侧 logic.select 一致）。 */
    fun sendSelect(index: Int) = send("""{"type":"select","index":$index}""")
    fun sendNextPage() = send("""{"type":"next_page"}""")
    fun sendPrevPage() = send("""{"type":"prev_page"}""")

    /**
     * partial-write 循环（镜像 Rust 侧 candidate_io.rs）：写满或失败为止；失败返回 false。
     * 整个「取句柄 + 写」持 [connLock]，与 serveLoop 的「置 null + Disconnect + Close」
     * 互斥 —— 否则句柄号被复用后这里会写进下一条连接（Rust 侧同样在 conn 锁内写）。
     */
    private fun send(json: String): Boolean = synchronized(connLock) {
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
        true
    }
}
