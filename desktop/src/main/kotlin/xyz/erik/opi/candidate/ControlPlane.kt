// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Windows 控制面：状态栏/设置进程 ⇄ **候选窗进程**（broker）⇄ TSF。
//!
//! 为什么要有它：Windows 的引擎状态在 TSF DLL 里（每个宿主进程一份），而状态栏/
//! 设置是另外的 JVM 进程；TSF 只与**候选窗进程**保持候选管道连接。于是候选窗进程
//! 当 broker：本文件开第二条本机命名管道 `\\.\pipe\opi-control` 收桌面请求，转成
//! 候选管道上的 `ctl` 请求，把 TSF 的 `ctl_result` 原样回给调用方。
//!
//! ```text
//! 状态栏/设置 --(本文件 client)--> \\.\pipe\opi-control --(PipeServer.requestCtl)-->
//! \\.\pipe\opi --> TSF（crates/tsf-opi/src/ctl.rs 执行）--> 原路返回
//! 请求：{"op":"state"} / {"action":"punct","op":"action"} /
//!       {"op":"search","query":"smile"} / {"op":"commit","text":"😄"} /
//!       {"op":"reload"}（设置窗保存后让 TSF 重读 settings.ini + 词库）
//! 应答：TSF 的 ctl_result 行，原样（{"id":1,"ok":true,"state":"..."}）
//!       broker 自己的失败：{"error":"…","ok":false,"type":"ctl_result"}
//! ```
//!
//! 信任边界：**本机同一用户会话**。PIPE_REJECT_REMOTE_CLIENTS 只挡远程 SMB；
//! 本地任意进程都能连（与候选管道同一条边界），所以两件事必须做：服务端串行、
//! 单实例、请求有长度上限；真正的 DoS 防线是「一次连接只处理一行、2s 读超时」。
//! 这里**不是**权限边界，别把它当沙箱。
//!
//! 失败语义（全部显式，不假装成功）：候选进程没起 / TSF 没连 / 超时 → 调用方拿到
//! 一句可显示的失败；状态读失败 → 中性 [ImeState]（状态栏既有裁决）。
//!
//! ⚠️ 本机（Linux）只能编译/单测纯解析部分；JNA 管道读写**未在 Windows 实跑**，
//! 交付时如实列为残余项。

package xyz.erik.opi.candidate

import com.sun.jna.Native
import com.sun.jna.platform.win32.Kernel32
import com.sun.jna.platform.win32.WinBase
import com.sun.jna.platform.win32.WinNT
import com.sun.jna.ptr.IntByReference
import java.io.ByteArrayOutputStream
import kotlin.concurrent.thread

// ---------- 常量 ----------

/** broker 管道名（TSF 侧不连它；只在桌面进程之间）。 */
private const val CONTROL_PIPE_NAME = "\\\\.\\pipe\\opi-control"
private const val PIPE_ACCESS_DUPLEX = 0x3
private const val FILE_FLAG_FIRST_PIPE_INSTANCE = 0x80000
private const val PIPE_TYPE_BYTE = 0x0
private const val PIPE_READMODE_BYTE = 0x0
private const val PIPE_WAIT = 0x0
private const val PIPE_REJECT_REMOTE_CLIENTS = 0x00000008
private const val PIPE_BUF = 4096
private const val MAX_INSTANCES = 1
private const val ERROR_PIPE_CONNECTED = 535
private const val ERROR_PIPE_BUSY = 231

/** One-shot 客户端要的 Win32 位（CreateFile 的读写/打开方式/属性）。 */
private const val GENERIC_READ_WRITE = -1073741824 // 0xC0000000
private const val OPEN_EXISTING = 3

/** 单条控制请求/应答上限（状态/搜索/直插都远小于此；超限 = 协议错乱 → 拒绝）。 */
private const val MAX_CONTROL_BYTES = 64 * 1024
/** 服务端读请求的最长等待（本地客户端正常在毫秒级发出整行）。 */
private const val REQUEST_READ_TIMEOUT_MS = 2000L
/** 状态轮询短一点（状态栏每秒一拍），动作允许 TSF 稍慢。 */
private const val STATE_TIMEOUT_MS = 1500L
private const val ACTION_TIMEOUT_MS = 3000L

/** 动作白名单：与 `crates/tsf-opi/src/ctl.rs` 的 match 分支一一对应。 */
private val CONTROL_ACTIONS =
    setOf("lang", "trad", "punct", "fullwidth", "englishpro", "mode", "clearuserwords")

// ---------- 纯协议（Linux 可单测） ----------

internal data class ControlReply(
    val ok: Boolean,
    val state: String?,
    val token: String?,
    val texts: List<String>?,
    val error: String?,
)

/** 解析 TSF/broker 的一条 ctl_result。形状不对 → null（调用方按失败处理，别降级成空结果）。 */
internal fun parseControlReply(line: String?): ControlReply? {
    val obj = line?.let(::parseLine) ?: return null
    if ((obj["type"] as? JVal.JStr)?.v != "ctl_result") return null
    val ok = (obj["ok"] as? JVal.JBool)?.v ?: return null
    val texts = (obj["texts"] as? JVal.JArr)?.v?.map { (it as? JVal.JStr)?.v ?: return null }
    return ControlReply(
        ok = ok,
        state = (obj["state"] as? JVal.JStr)?.v,
        token = (obj["token"] as? JVal.JStr)?.v,
        texts = texts,
        error = (obj["error"] as? JVal.JStr)?.v,
    )
}

internal fun controlStateRequest(): String = """{"op":"state"}"""
internal fun controlActionRequest(action: String): String =
    """{"action":${jsonString(action)},"op":"action"}"""
internal fun controlSearchRequest(query: String): String =
    """{"op":"search","query":${jsonString(query)}}"""
internal fun controlCommitRequest(text: String): String =
    """{"op":"commit","text":${jsonString(text)}}"""
internal fun controlReloadRequest(): String = """{"op":"reload"}"""

/** broker 的失败应答（与 TSF 的 ctl_result 同形状，调用方只有一条解析路）。 */
internal fun controlErrorLabel(why: String): String =
    """{"error":${jsonString(why)},"ok":false,"type":"ctl_result"}"""

/** broker 对一行请求的判定：转发给 TSF，或直接拒绝（不碰管道，可单测）。 */
internal sealed interface ControlDispatch {
    class Forward(val opFields: String, val timeoutMs: Long) : ControlDispatch
    class Reject(val reply: String) : ControlDispatch
}

/**
 * 校验请求 → 转成 TSF 的 op 字段串（id/type 由 [PipeServer.requestCtl] 补）。
 * 白名单与长度上限在这里，**别只写在 client 侧**：本机任意进程都能连这条管道
 * （信任边界见文件头），服务端必须自己判。
 */
internal fun controlDispatch(line: String): ControlDispatch {
    val obj = parseLine(line) ?: return ControlDispatch.Reject(controlErrorLabel("请求不是 JSON 对象"))
    return when (val op = (obj["op"] as? JVal.JStr)?.v) {
        "state" -> ControlDispatch.Forward("\"op\":\"state\"", STATE_TIMEOUT_MS)
        "action" -> {
            val action = (obj["action"] as? JVal.JStr)?.v
                ?: return ControlDispatch.Reject(controlErrorLabel("缺 action"))
            if (action !in CONTROL_ACTIONS) {
                return ControlDispatch.Reject(controlErrorLabel("未知动作：$action"))
            }
            ControlDispatch.Forward("\"action\":${jsonString(action)},\"op\":\"action\"", ACTION_TIMEOUT_MS)
        }
        "search" -> {
            val query = (obj["query"] as? JVal.JStr)?.v
                ?: return ControlDispatch.Reject(controlErrorLabel("缺 query"))
            // TSF 侧上限是 **UTF-8 字节** 256（ctl.rs 的 MAX_QUERY）。按字符判会让
            // 100 个汉字（300 字节）通过 broker、被 TSF 静默丢弃 → 只能等超时。
            if (query.isEmpty() || query.toByteArray(Charsets.UTF_8).size > 256) {
                return ControlDispatch.Reject(controlErrorLabel("搜索词为空或过长"))
            }
            ControlDispatch.Forward("\"op\":\"search\",\"query\":${jsonString(query)}", ACTION_TIMEOUT_MS)
        }
        "commit" -> {
            val text = (obj["text"] as? JVal.JStr)?.v
                ?: return ControlDispatch.Reject(controlErrorLabel("缺 text"))
            // Rust 侧按 **UTF-8 字节**上限 4096；这里按字符 1024（最坏 4 字节/字符）。
            if (text.isEmpty() || text.length > 1024) {
                return ControlDispatch.Reject(controlErrorLabel("文本为空或过长"))
            }
            ControlDispatch.Forward("\"op\":\"commit\",\"text\":${jsonString(text)}", ACTION_TIMEOUT_MS)
        }
        // 无参数：路径由 TSF 按 LOCALAPPDATA 现推（请求方给不了别的路径）。
        "reload" -> ControlDispatch.Forward("\"op\":\"reload\"", ACTION_TIMEOUT_MS)
        else -> ControlDispatch.Reject(controlErrorLabel("未知控制操作：$op"))
    }
}

// ---------- 客户端（状态栏/设置进程的一问一答） ----------

/**
 * 连 broker、发一行、读到 EOF。返回应答首行；连不上/超时/超长 → null。
 *
 * 服务端处理完**主动断开**（一次连接一条请求），所以读循环以 EOF 结束，不需要
 * 单独的分帧超时；最坏等待由 broker 自己的 requestCtl 超时兜住。
 * 只在 Windows 上调用（调用方先判 [isWindows]）。
 */
internal fun controlCall(request: String, timeoutMs: Long = ACTION_TIMEOUT_MS + 1000): String? {
    val deadline = System.nanoTime() + timeoutMs * 1_000_000L
    var pipe: WinNT.HANDLE? = null
    while (true) {
        val h = Kernel32.INSTANCE.CreateFile(
            CONTROL_PIPE_NAME, GENERIC_READ_WRITE, 0, null, OPEN_EXISTING, 0, null,
        )
        if (h != null && h != WinBase.INVALID_HANDLE_VALUE) {
            pipe = h
            break
        }
        // 忙 = broker 正在服务另一条连接（单实例转一圈很快）；文件不存在 = 候选进程没起。
        if (Native.getLastError() != ERROR_PIPE_BUSY || System.nanoTime() >= deadline) return null
        Thread.sleep(20)
    }
    // 循环只有「赋了非空句柄后 break」与「return null」两种出口 ⇒ 这里必非空。
    val h = pipe
    return try {
        val bytes = (request + "\n").toByteArray(Charsets.UTF_8)
        var off = 0
        while (off < bytes.size) {
            val written = IntByReference()
            if (!Kernel32.INSTANCE.WriteFile(h, bytes.copyOfRange(off, bytes.size), bytes.size - off, written, null)) {
                return null
            }
            if (written.value <= 0) return null
            off += written.value
        }
        val out = ByteArrayOutputStream()
        val buf = ByteArray(PIPE_BUF)
        while (out.size() <= MAX_CONTROL_BYTES) {
            val n = IntByReference()
            if (!Kernel32.INSTANCE.ReadFile(h, buf, buf.size, n, null) || n.value <= 0) break
            out.write(buf, 0, n.value)
        }
        if (out.size() > MAX_CONTROL_BYTES) return null
        out.toString(Charsets.UTF_8.name()).lineSequence().firstOrNull()?.trim()
    } finally {
        Kernel32.INSTANCE.CloseHandle(h)
    }
}

/** Windows 状态读。通道失败 → null（调用方回落中性 [ImeState]）。 */
internal fun controlImeState(): ImeState? {
    val reply = parseControlReply(controlCall(controlStateRequest(), STATE_TIMEOUT_MS + 500))
    if (reply == null || !reply.ok) return null
    return reply.state?.let(::parseImeState)
}

/** 动作。返回 null = 成功；非 null = 给用户看的原因。 */
internal fun controlAction(action: String, timeoutMs: Long = ACTION_TIMEOUT_MS): String? {
    val reply = parseControlReply(controlCall(controlActionRequest(action), timeoutMs + 1000))
        ?: return "连不上输入法（候选窗进程未运行，或 TSF 未连接）"
    return if (reply.ok) null else reply.error ?: "输入法拒绝了该动作"
}

/** 符号页直插。null = 已入上屏队列；非 null = 失败原因。 */
internal fun controlCommit(text: String): String? = controlActionReply(controlCommitRequest(text))

private fun controlActionReply(request: String): String? {
    val reply = parseControlReply(controlCall(request))
        ?: return "连不上输入法（候选窗进程未运行，或 TSF 未连接）"
    return if (reply.ok) null else reply.error ?: "输入法拒绝了该请求"
}

/** 符号搜索。坏载荷必须是 Failed，不能降级成空命中。 */
internal fun controlSearch(query: String): SymbolSearch {
    val reply = parseControlReply(controlCall(controlSearchRequest(query)))
        ?: return SymbolSearch.Failed("连不上输入法（候选窗进程未运行，或 TSF 未连接）")
    if (!reply.ok) return SymbolSearch.Failed(reply.error ?: "输入法拒绝了搜索请求")
    val texts = reply.texts ?: return SymbolSearch.Failed("搜索应答缺 texts")
    return SymbolSearch.Hits(texts)
}

/** 模式循环：只认三个 token；其余（含失败）回空串，与 Linux 契约一致。 */
internal fun controlMode(): String {
    val reply = parseControlReply(controlCall(controlActionRequest("mode")))
    return reply?.takeIf { it.ok }?.token?.takeIf { it in MODE_TOKENS } ?: ""
}

/**
 * 让 TSF 重读 settings.ini + 词库目录。true = 已重读；false = 没连上/被拒
 * （文件已由调用方写好，下次激活仍会生效 —— 所以这里是布尔而不是错误文案）。
 */
internal fun controlReload(): Boolean =
    parseControlReply(controlCall(controlReloadRequest()))?.ok == true

// ---------- broker（候选窗进程内的服务端） ----------

/**
 * `\\.\pipe\opi-control` 的服务端：一次连接一条请求，串行处理。
 * 接到后交给 [PipeServer.requestCtl] 转发给 TSF，把应答原样写回并断开。
 * 专用线程 ⇒ 不阻塞候选窗 UI，也不阻塞候选管道的读线程。
 */
internal class ControlServer(private val pipes: PipeServer) {
    fun start() {
        thread(isDaemon = true, name = "opi-control") { serveLoop() }
    }

    private fun serveLoop() {
        while (true) {
            var pipe: WinNT.HANDLE? = null
            try {
                val created = createPipe()
                if (created == null) {
                    Thread.sleep(500)
                    continue
                }
                pipe = created
                if (!connect(created)) continue
                val request = readRequest(created) ?: continue
                val reply = handle(request)
                writeReply(created, reply)
            } catch (t: Throwable) {
                if (t is InterruptedException) {
                    Thread.currentThread().interrupt()
                    return
                }
            } finally {
                pipe?.let {
                    Kernel32.INSTANCE.DisconnectNamedPipe(it)
                    Kernel32.INSTANCE.CloseHandle(it)
                }
            }
        }
    }

    private fun handle(line: String): String =
        when (val d = controlDispatch(line)) {
            is ControlDispatch.Forward -> forward(d.opFields, d.timeoutMs)
            is ControlDispatch.Reject -> d.reply
        }

    private fun forward(opFields: String, timeoutMs: Long): String =
        pipes.requestCtl(opFields, timeoutMs)
            ?: controlErrorLabel("输入法未连接或应答超时")

    private fun createPipe(): WinNT.HANDLE? {
        val h = Kernel32.INSTANCE.CreateNamedPipe(
            CONTROL_PIPE_NAME,
            PIPE_ACCESS_DUPLEX or FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE or PIPE_READMODE_BYTE or PIPE_WAIT or PIPE_REJECT_REMOTE_CLIENTS,
            MAX_INSTANCES, PIPE_BUF, PIPE_BUF, 0, null,
        )
        return if (h == null || h == WinBase.INVALID_HANDLE_VALUE) null else h
    }

    private fun connect(pipe: WinNT.HANDLE): Boolean {
        if (Kernel32.INSTANCE.ConnectNamedPipe(pipe, null)) return true
        // 与候选管道同一判定：ERROR_PIPE_CONNECTED = 正常竞态；err==0 = 读不到错误码，放行自愈。
        val err = Native.getLastError()
        return err == ERROR_PIPE_CONNECTED || err == 0
    }

    /** 轮询等待整行（2s 上限）：被本机进程连上却不发请求时，不能把单实例管道永远占住。 */
    private fun readRequest(pipe: WinNT.HANDLE): String? {
        val deadline = System.nanoTime() + REQUEST_READ_TIMEOUT_MS * 1_000_000L
        val pending = ByteArrayOutputStream()
        val buf = ByteArray(PIPE_BUF)
        while (pending.size() <= MAX_CONTROL_BYTES) {
            val avail = IntByReference()
            if (!Kernel32.INSTANCE.PeekNamedPipe(pipe, null, 0, null, avail, null)) return null
            if (avail.value > 0) {
                val n = IntByReference()
                if (!Kernel32.INSTANCE.ReadFile(pipe, buf, minOf(buf.size, avail.value), n, null) || n.value <= 0) {
                    return null
                }
                pending.write(buf, 0, n.value)
                val bytes = pending.toByteArray()
                val nl = bytes.indexOf('\n'.code.toByte())
                if (nl >= 0) return String(bytes, 0, nl, Charsets.UTF_8)
            } else if (System.nanoTime() >= deadline) {
                return null
            } else {
                Thread.sleep(10)
            }
        }
        return null
    }

    private fun writeReply(pipe: WinNT.HANDLE, reply: String) {
        val bytes = (reply + "\n").toByteArray(Charsets.UTF_8)
        var off = 0
        while (off < bytes.size) {
            val written = IntByReference()
            if (!Kernel32.INSTANCE.WriteFile(pipe, bytes.copyOfRange(off, bytes.size), bytes.size - off, written, null)) {
                return
            }
            if (written.value <= 0) return
            off += written.value
        }
    }
}
