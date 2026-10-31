// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置窗 ⇄ 状态栏宿主 的**本机回环**控制通道。
//!
//! 为什么需要它：剪贴板内核挂在 `--statusbar` 常驻进程里（句柄不可跨线程，更不能跨进程）；
//! 设置窗可能是另一个进程（`--settings`）。两端靠一个只监听 loopback 的 TCP 端口 +
//! 端点文件（含随机 token）通信；端点文件权限按 600 收紧（Windows 上忽略）。
//!
//! 隐私：命令 `send` 会带**手动发送**的剪贴板文本（Wayland 的唯一通路）；应答与状态
//! 快照**只含元数据**，绝不回传文本。

package xyz.erik.opi.candidate

import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.SocketTimeoutException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.attribute.PosixFilePermissions
import java.security.MessageDigest

internal const val CLIP_CONTROL_MAX_LINE = 1024 * 1024
private const val CLIP_CONTROL_READ_MS = 2000
private const val CLIP_CONTROL_TIMEOUT_MS = 1500

/**
 * shortcut: 内核的 256 KiB 上限按 UTF-8 字节算，这里用 UTF-16 长度做便宜预检；
 * 非 ASCII 文本可能过宽，最终由 core 拒绝并回报准确字节数。
 */
internal const val CLIP_TEXT_MAX_CHARS = 256 * 1024

internal fun clipDataDir(): Path = dictsDir().parent.resolve("clip")

internal fun clipControlPath(): Path = clipDataDir().resolve("control.json")

internal sealed interface ClipCommand {
    data class SetEnabled(val on: Boolean) : ClipCommand
    data class Pair(val peer: String) : ClipCommand
    data class Confirm(val peer: String, val accept: Boolean) : ClipCommand
    data class Forget(val peer: String) : ClipCommand

    /** 手动发送（Wayland）：文本只在内存里过一手，不落盘、不记日志。 */
    data class SendText(val text: String) : ClipCommand
}

private fun writeJson(socket: Socket, json: String) {
    socket.getOutputStream().write((json + "\n").toByteArray(Charsets.UTF_8))
    socket.getOutputStream().flush()
}

private fun replyError(socket: Socket, message: String) {
    writeJson(socket, "{\"ok\":false,\"error\":${jsonString(message)}}")
}

/**
 * 本机控制服务器。所有命令都只**入队**（真正的 FFI 调用在宿主剪贴板线程上），
 * `status` 直接读宿主维护的不可变快照。
 */
internal class ClipControlServer(
    private val controlFile: Path,
    private val token: String,
    private val status: () -> String,
    private val enqueue: (ClipCommand) -> Unit,
) {
    @Volatile private var closed = false
    private var server: ServerSocket? = null
    private var thread: Thread? = null

    fun start() {
        thread = Thread({ run() }, "opi-clip-control").apply {
            isDaemon = true
            start()
        }
    }

    private fun run() {
        val ss = try {
            ServerSocket(0, 16, InetAddress.getLoopbackAddress()).also {
                it.soTimeout = 250
                server = it
            }
        } catch (_: Exception) {
            return
        }
        try {
            writeEndpoint(ss.localPort)
        } catch (_: Exception) {
            runCatching { ss.close() }
            return
        }
        try {
            while (!closed) {
                val socket = try {
                    ss.accept()
                } catch (_: SocketTimeoutException) {
                    continue
                } catch (_: Exception) {
                    if (closed) break else continue
                }
                socket.use { handle(it) }
            }
        } finally {
            runCatching { Files.deleteIfExists(controlFile) }
            runCatching { ss.close() }
        }
    }

    private fun writeEndpoint(port: Int) {
        Files.createDirectories(controlFile.parent)
        writeTextAtomically(controlFile, "{\"port\":$port,\"token\":${jsonString(token)}}\n")
        // 同用户的本机进程才该读到 token；Windows 无 POSIX 权限，失败无妨（目录 ACL）。
        runCatching {
            Files.setPosixFilePermissions(
                controlFile,
                PosixFilePermissions.fromString("rw-------"),
            )
        }
    }

    private fun handle(socket: Socket) {
        socket.soTimeout = CLIP_CONTROL_READ_MS
        val line = try {
            BufferedReader(InputStreamReader(socket.getInputStream(), Charsets.UTF_8)).readLine()
        } catch (_: Exception) {
            return
        } ?: return
        if (line.length > CLIP_CONTROL_MAX_LINE) {
            replyError(socket, "控制请求过大")
            return
        }
        val m = parseLine(line) ?: run {
            replyError(socket, "控制请求不是合法 JSON")
            return
        }
        val given = (m["token"] as? JVal.JStr)?.v
        if (given == null || !MessageDigest.isEqual(
                given.toByteArray(Charsets.UTF_8),
                token.toByteArray(Charsets.UTF_8),
            )
        ) {
            replyError(socket, "token 无效")
            return
        }
        when ((m["cmd"] as? JVal.JStr)?.v) {
            "status" -> writeJson(socket, status())
            "enable" -> {
                val on = (m["on"] as? JVal.JBool)?.v
                if (on == null) return replyError(socket, "enable 缺少 on")
                enqueue(ClipCommand.SetEnabled(on))
                writeJson(socket, "{\"ok\":true}")
            }
            "pair", "forget" -> {
                val peer = peerArg(m) ?: return replyError(socket, "peer 必须是 32 位十六进制")
                enqueue(
                    if ((m["cmd"] as? JVal.JStr)?.v == "pair") {
                        ClipCommand.Pair(peer)
                    } else {
                        ClipCommand.Forget(peer)
                    },
                )
                writeJson(socket, "{\"ok\":true}")
            }
            "confirm" -> {
                val peer = peerArg(m) ?: return replyError(socket, "peer 必须是 32 位十六进制")
                val accept = (m["accept"] as? JVal.JBool)?.v
                    ?: return replyError(socket, "confirm 缺少 accept")
                enqueue(ClipCommand.Confirm(peer, accept))
                writeJson(socket, "{\"ok\":true}")
            }
            "send" -> {
                val text = (m["text"] as? JVal.JStr)?.v
                    ?: return replyError(socket, "send 缺少 text")
                if (text.length > CLIP_TEXT_MAX_CHARS) {
                    return replyError(socket, "文本超过 256 KiB 上限")
                }
                enqueue(ClipCommand.SendText(text))
                writeJson(socket, "{\"ok\":true}")
            }
            "stop" -> {
                enqueue(ClipCommand.SetEnabled(false))
                writeJson(socket, "{\"ok\":true}")
            }
            else -> replyError(socket, "未知命令")
        }
    }

    private fun peerArg(m: Map<String, JVal>): String? =
        (m["peer"] as? JVal.JStr)?.v?.takeIf { it.matches(Regex("[0-9a-fA-F]{32}")) }

    fun close() {
        closed = true
        runCatching { server?.close() }
        runCatching { thread?.join(500) }
        runCatching { Files.deleteIfExists(controlFile) }
    }
}

// ---------- 客户端（设置窗侧） ----------

private data class ClipEndpoint(val port: Int, val token: String)

private fun readClipEndpoint(): ClipEndpoint? {
    val text = try {
        Files.readString(clipControlPath())
    } catch (_: Exception) {
        return null
    }
    val m = parseLine(text) ?: return null
    val port = (m["port"] as? JVal.JNum)?.v?.toInt() ?: return null
    val token = (m["token"] as? JVal.JStr)?.v ?: return null
    if (port !in 1..65535 || token.length < 16) return null
    return ClipEndpoint(port, token)
}

internal object ClipClient {
    /** 发送一条带 token 的请求；失败（宿主没起/超时）→ null。 */
    private fun request(body: String): String? {
        val ep = readClipEndpoint() ?: return null
        return try {
            Socket().use { s ->
                s.connect(InetSocketAddress(InetAddress.getLoopbackAddress(), ep.port), 600)
                s.soTimeout = CLIP_CONTROL_TIMEOUT_MS
                s.getOutputStream().write((body + "\n").toByteArray(Charsets.UTF_8))
                s.getOutputStream().flush()
                BufferedReader(InputStreamReader(s.getInputStream(), Charsets.UTF_8)).readLine()
            }
        } catch (_: Exception) {
            null
        }
    }

    private fun command(token: String, fields: String): Boolean {
        val answer = request(
            "{\"token\":${jsonString(token)},\"cmd\":$fields}",
        ) ?: return false
        return (parseLine(answer)?.get("ok")) == JVal.JBool(true)
    }

    private fun command(fields: String): Boolean {
        val ep = readClipEndpoint() ?: return false
        return command(ep.token, fields)
    }

    fun status(): ClipStatus? {
        val ep = readClipEndpoint() ?: return null
        return request("{\"token\":${jsonString(ep.token)},\"cmd\":\"status\"}")
            ?.let(::parseClipStatus)
    }

    fun setEnabled(on: Boolean): Boolean = command("\"enable\",\"on\":$on")

    fun pair(peer: String): Boolean = command("\"pair\",\"peer\":${jsonString(peer)}")

    fun confirm(peer: String, accept: Boolean): Boolean =
        command("\"confirm\",\"peer\":${jsonString(peer)},\"accept\":$accept")

    fun forget(peer: String): Boolean = command("\"forget\",\"peer\":${jsonString(peer)}")

    fun sendText(text: String): Boolean = command("\"send\",\"text\":${jsonString(text)}")

    fun stop(): Boolean = command("\"stop\"")
}
