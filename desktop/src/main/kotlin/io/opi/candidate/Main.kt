//! C3：候选窗（Compose Desktop / JVM）—— OPI 拼音输入法 Windows 桌面候选窗。
//!
//! 【线协议：NDJSON over named pipe】（与 crates/tsf-opi/src/candidate_io.rs
//! 头注释互为镜像，改协议须同步两处）
//!
//! ```text
//! 管道名：\\.\pipe\opi-candidates（单实例，字节流模式，'\n' 分帧，UTF-8）
//! 角色：本窗口为 SERVER 监听管道；TSF 插件进程为 CLIENT（窗口启动后连接）。
//!
//! TSF(CLIENT) → 本窗(SERVER)：
//!   show      {"type":"show","buffer":"ni","candidates":["你","尼",...],
//!              "page":1,"page_count":3,"mode":"pinyin"}
//!             （page/page_count 为 1 起；candidates 为当前页文本）
//!   hide      {"type":"hide"}
//!   position  {"type":"position","x":120,"y":340}  // caret 提示（骨架降级固定位置）
//!
//! 本窗(SERVER) → TSF(CLIENT)：
//!   select    {"type":"select","index":0}   // 用户点击第 index（页内 0 起）候选
//!   next_page {"type":"next_page"} / {"type":"prev_page"}
//! ```
//!
//! 传输实现：JNA（net.java.dev.jna 5.6.0，本机缓存）直调 kernel32 ——
//! CreateNamedPipeW/ConnectNamedPipe 为 raw Function（jna-platform 的 Kernel32
//! 接口不含二者），ReadFile/WriteFile/DisconnectNamedPipe/CloseHandle 用
//! Kernel32.INSTANCE。jna-platform 是跨平台 jar，Linux 上可编译（kernel32
//! 仅 Windows 存在 —— 验收阶段在 Windows 上运行）。
//!
//! 窗口行为：无边框 + 置顶 + 不抢焦点（输入法候选窗语义）；初始隐藏，
//! show 消息到达才显示；position 消息 → 跟随 caret（缺省固定默认位置）。
//! 翻页：本地即时翻转（响应性）+ 同时发消息给 TSF（权威页码以 TSF 回发的
//! show 为准）。

package io.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.WindowState
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import com.sun.jna.Function
import com.sun.jna.WString
import com.sun.jna.platform.win32.Kernel32
import com.sun.jna.platform.win32.WinBase
import com.sun.jna.platform.win32.WinNT
import com.sun.jna.ptr.IntByReference
import kotlin.concurrent.thread

// ---------- 自绘配色（无 material3 依赖，见 build.gradle.kts 注释） ----------

private val BgColor = Color(0xFF2D2D2D)
private val TextColor = Color(0xFFE8E8E8)
private val DimColor = Color(0xFF9E9E9E)
private val AccentColor = Color(0xFF4C8DFF)
private val BtnColor = Color(0xFF3A3A3A)

// ---------- 管道常量 ----------

/** 管道名（与 Rust 侧 candidate_io.rs 的 PIPE_NAME 一致）。 */
private const val PIPE_NAME = "\\\\.\\pipe\\opi-candidates"
private const val PIPE_ACCESS_DUPLEX = 0x3
private const val PIPE_TYPE_BYTE = 0x0 // 字节流模式 + PIPE_WAIT（阻塞）
private const val PIPE_READMODE_BYTE = 0x0
private const val PIPE_WAIT = 0x0
private const val PIPE_BUF = 4096
private const val MAX_INSTANCES = 1
private const val FIXED_X = 200
private const val FIXED_Y = 320
private const val MAX_CANDIDATES = 8

// ---------- 窗口状态（snapshot state，服务器线程跨线程写入） ----------

/** 候选窗渲染状态：由管道服务器线程更新，Compose 层读取。 */
class CandidateModel {
    var visible by mutableStateOf(false)
    var buffer by mutableStateOf("")
    var candidates by mutableStateOf(emptyList<String>())
    var page by mutableIntStateOf(1)
    var pageCount by mutableIntStateOf(1)
    var mode by mutableStateOf("pinyin")
    var x by mutableIntStateOf(FIXED_X)
    var y by mutableIntStateOf(FIXED_Y)
}

// ---------- 最小 JSON 解析（协议仅需字符串/数字/字符串数组，零额外依赖） ----------

private sealed interface JVal {
    data class JStr(val v: String) : JVal
    data class JNum(val v: Long) : JVal
    data class JArr(val v: List<JVal>) : JVal
    data class JObj(val v: Map<String, JVal>) : JVal
}

/** 递归下降解析器：支持对象/字符串数组/数字/转义（\" \\uXXXX 等）。 */
private class JParse(private val s: String) {
    private var i = 0

    fun parse(): JVal? {
        skipWs()
        val v = parseValue()
        skipWs()
        return v
    }

    private fun skipWs() {
        while (i < s.length && s[i].isWhitespace()) i++
    }


    private fun parseValue(): JVal? {
        if (i >= s.length) return null
        return when (s[i]) {
            '"' -> JVal.JStr(parseString() ?: return null)
            '{' -> parseObject()
            '[' -> parseArray()
            else -> parseNumber()
        }
    }

    private fun parseObject(): JVal.JObj? {
        expect('{') ?: return null
        val out = LinkedHashMap<String, JVal>()
        skipWs()
        if (i < s.length && s[i] == '}') { i++; return JVal.JObj(out) }
        while (true) {
            skipWs()
            val key = parseString() ?: return null
            skipWs()
            expect(':') ?: return null
            skipWs()
            out[key] = parseValue() ?: return null
            skipWs()
            if (i >= s.length) return null
            when (s[i]) {
                ',' -> i++
                '}' -> { i++; return JVal.JObj(out) }
                else -> return null
            }
        }
    }

    private fun parseArray(): JVal.JArr? {
        expect('[') ?: return null
        val out = ArrayList<JVal>()
        skipWs()
        if (i < s.length && s[i] == ']') { i++; return JVal.JArr(out) }
        while (true) {
            skipWs()
            out.add(parseValue() ?: return null)
            skipWs()
            if (i >= s.length) return null
            when (s[i]) {
                ',' -> i++
                ']' -> { i++; return JVal.JArr(out) }
                else -> return null
            }
        }
    }

    private fun parseString(): String? {
        expect('"') ?: return null
        val sb = StringBuilder()
        while (i < s.length) {
            when (val c = s[i++]) {
                '"' -> return sb.toString()
                '\\' -> {
                    if (i >= s.length) return null
                    when (val e = s[i++]) {
                        '"' -> sb.append('"')
                        '\\' -> sb.append('\\')
                        '/' -> sb.append('/')
                        'n' -> sb.append('\n')
                        't' -> sb.append('\t')
                        'r' -> sb.append('\r')
                        'u' -> {
                            if (i + 4 > s.length) return null
                            sb.append(s.substring(i, i + 4).toInt(16).toChar())
                            i += 4
                        }
                        else -> return null
                    }
                }
                else -> sb.append(c)
            }
        }
        return null
    }

    private fun parseNumber(): JVal.JNum? {
        val start = i
        while (i < s.length && (s[i].isDigit() || s[i] == '-')) i++
        if (i == start) return null
        return JVal.JNum(s.substring(start, i).toLong())
    }

    private fun expect(c: Char): Boolean = if (i < s.length && s[i] == c) { i++; true } else false
}

/** 解析一行消息为对象（失败返回 null）。 */
private fun parseLine(text: String): Map<String, JVal>? =
    (JParse(text).parse() as? JVal.JObj)?.v

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
                WString(PIPE_NAME), PIPE_ACCESS_DUPLEX,
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
        val pending = StringBuilder() // '\n' 分帧的残留
        while (true) {
            val n = IntByReference()
            // 字节流阻塞读：失败或 0 字节 = 客户端断开。
            if (!Kernel32.INSTANCE.ReadFile(pipe, buf, buf.size, n, null) || n.value <= 0) return
            pending.append(String(buf, 0, n.value, Charsets.UTF_8))
            while (true) {
                val nl = pending.indexOf("\n")
                if (nl < 0) break
                handleLine(pending.substring(0, nl))
                pending.delete(0, nl + 1)
            }
        }
    }

    private fun handleLine(line: String) {
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
    }

    /** 点击候选 → TSF 提交（index 页内 0 起，与 TSF 侧 logic.select 一致）。 */
    fun sendSelect(index: Int) = send("""{"type":"select","index":$index}""")
    fun sendNextPage() = send("""{"type":"next_page"}""")
    fun sendPrevPage() = send("""{"type":"prev_page"}""")

    private fun send(json: String) {
        val pipe = lastClientPipe ?: return
        val bytes = (json + "\n").toByteArray(Charsets.UTF_8)
        val written = IntByReference()
        Kernel32.INSTANCE.WriteFile(pipe, bytes, bytes.size, written, null)
    }
}

// ---------- Compose UI ----------

fun main() = application {
    val model = remember { CandidateModel() }
    val server = remember { PipeServer(model).also { it.start() } }
    val windowState = rememberWindowState(width = 320.dp, height = 168.dp)

    Window(
        onCloseRequest = ::exitApplication,
        state = windowState,
        // 可见性跟随模型（初始隐藏，show 消息到达才显示）。
        visible = model.visible,
        undecorated = true,
        alwaysOnTop = true,
        resizable = false,
        focusable = false,
        title = "OPI 候选窗",
    ) {
        // 位置跟随（position 消息；缺省固定默认位置）。
        LaunchedEffect(model.x, model.y, model.visible) {
            if (model.visible) {
                windowState.position = WindowPosition(model.x.dp, model.y.dp)
            }
        }
        CandidatePanel(
            model = model,
            onSelect = server::sendSelect,
            onNext = server::sendNextPage,
            onPrev = server::sendPrevPage,
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CandidatePanel(
    model: CandidateModel,
    onSelect: (Int) -> Unit,
    onNext: () -> Unit,
    onPrev: () -> Unit,
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(BgColor)
            .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        Column {
            // 首行：缓冲 + 模式标签
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    model.buffer,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Bold,
                    color = TextColor,
                )
                Spacer(Modifier.width(8.dp))
                Text(modeLabel(model.mode), fontSize = 12.sp, color = DimColor)
            }
            Spacer(Modifier.height(6.dp))
            // 候选列表（最多 8 个/页；点击 → TSF 提交）
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                model.candidates.take(MAX_CANDIDATES).forEachIndexed { index, text ->
                    CandidateItem(index + 1, text) { onSelect(index) }
                }
            }
            Spacer(Modifier.height(4.dp))
            // 底行：页码 + 翻页
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                PageButton("‹", enabled = model.page > 1, onClick = onPrev)
                Text(
                    "${model.page}/${model.pageCount}",
                    fontSize = 12.sp,
                    color = DimColor,
                )
                PageButton("›", enabled = model.page < model.pageCount, onClick = onNext)
            }
        }
    }
}

@Composable
private fun CandidateItem(index: Int, text: String, onClick: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.clickable(onClick = onClick),
    ) {
        Text("$index.", fontSize = 13.sp, color = DimColor)
        Text(text, fontSize = 17.sp, color = TextColor)
    }
}

/** 翻页按钮：无 material3 的 TextButton 平替。 */
@Composable
private fun PageButton(text: String, enabled: Boolean, onClick: () -> Unit) {
    val color = if (enabled) AccentColor else DimColor
    Box(
        modifier = Modifier
            .background(BtnColor)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 2.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, fontSize = 16.sp, color = color)
    }
}

/** 模式 → 中文标签（协议字符串见 Rust 侧 mode_str）。 */
private fun modeLabel(mode: String): String = when (mode) {
    "english" -> "英文"
    "number" -> "数字"
    "symbol" -> "符号"
    else -> "拼音"
}
