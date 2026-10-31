// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语音输入宿主（2026-11-02）：Java Sound 采集 → JNA → `libopi_asr`。
//!
//! 线程形状与 ClipService 同一条：**一个专用线程**独占 FFI 句柄（create/load/feed/
//! finish/cancel/poll/destroy 全在它上面），UI 只通过命令队列触发、读 Compose 状态。
//! 采集线程是第二条：`TargetDataLine` 读 100 ms PCM 块丢进同一个命令队列。
//! `opi_asr_feed_pcm` 非阻塞（队列满返回 -5）⇒ 遇到 -5 短暂重试，不丢整段录音。
//!
//! 隐私边界（用户要求）：音频只在内存里过一遍，**不落盘、不写日志**；正文只出现在
//! 浮层与最终上屏，不打印。模型缺失/麦克风被占用/库没装都折成浮层里的可读错误。
//!
//! 路由只有一处：`SpeechLanguage.engine`（zh/auto → SenseVoice，en → Whisper），
//! Rust 侧 `Language::model_kind` 是同一套；本文件不另判语言。

package xyz.erik.opi.candidate

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.sun.jna.Memory
import com.sun.jna.Pointer
import java.nio.file.Path
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import javax.sound.sampled.AudioFormat
import javax.sound.sampled.AudioSystem
import javax.sound.sampled.DataLine
import javax.sound.sampled.LineUnavailableException
import javax.sound.sampled.TargetDataLine

internal enum class SpeechPhase { IDLE, PREPARING, RECORDING, RECOGNIZING, DONE, ERROR }

internal data class SpeechUi(
    val phase: SpeechPhase = SpeechPhase.IDLE,
    val engine: SpeechEngine = SpeechEngine.SENSEVOICE,
    /** 0..1，来自 Rust 的 `level` 事件（rms_db -60..0）。 */
    val level: Float = 0f,
    /** 识别中的临时文本 / 最终文本。 */
    val text: String = "",
    val error: String = "",
)

/** 状态机的纯转移表（单测钉住；浮层与宿主共用）。 */
internal enum class SpeechTransition { START, READY, STOP, FINAL, FAIL, CANCEL, DISMISS }

internal fun nextSpeechPhase(from: SpeechPhase, t: SpeechTransition): SpeechPhase = when (t) {
    SpeechTransition.START -> SpeechPhase.PREPARING
    // READY/STOP/FINAL 只接受「上一步确实在那一档」的转移：迟到的 final 不能把
    // 已经取消的会话重新点亮。
    SpeechTransition.READY -> if (from == SpeechPhase.PREPARING) SpeechPhase.RECORDING else from
    SpeechTransition.STOP -> if (from == SpeechPhase.RECORDING) SpeechPhase.RECOGNIZING else from
    SpeechTransition.FINAL -> if (from == SpeechPhase.RECOGNIZING) SpeechPhase.DONE else from
    SpeechTransition.FAIL -> SpeechPhase.ERROR
    SpeechTransition.CANCEL, SpeechTransition.DISMISS -> SpeechPhase.IDLE
}

private sealed interface SpeechCommand {
    data class Toggle(val commit: (String) -> String?) : SpeechCommand
    data class Audio(val bytes: ByteArray) : SpeechCommand
    data class AudioError(val message: String) : SpeechCommand
    data object Cancel : SpeechCommand
    data object Dismiss : SpeechCommand
}

private data class SpeechConfig(val enabled: Boolean = false, val language: SpeechLanguage = SpeechLanguage.AUTO)

private class LoadedAsr(
    val ffi: OpiAsrFfi,
    val engine: SpeechEngine,
    val handle: Pointer,
    /** `max_event_bytes + 1`：Rust 的 write_c_string 要求 JSON 长度严格小于 cap。 */
    val eventBuf: Memory,
    val eventCap: Int,
)

private class Session(val engine: SpeechEngine, val commit: (String) -> String?) {
    var deadlineMs: Long = 0L
}

/** `AsrConfig::from_json` 的口径；路径全部绝对（模型根由桌面自己定）。 */
internal fun asrConfigJson(root: Path, language: SpeechLanguage): String = buildString {
    append("{\"models_dir\":").append(jsonString(root.toAbsolutePath().toString()))
    append(",\"sense_voice_dir\":\"sensevoice\"")
    append(",\"whisper_dir\":\"whisper-small.en\"")
    append(",\"vad_model\":\"vad/silero_vad.onnx\"")
    append(",\"language\":").append(jsonString(language.value))
    append(",\"num_threads\":2,\"verify_models\":true}")
}

/** Java Sound 采集：16 kHz / 16 bit / mono / little-endian，专用读线程。 */
internal class AudioCapture(
    private val onChunk: (ByteArray) -> Unit,
    private val onError: (String) -> Unit,
) {
    @Volatile private var running = false
    private var line: TargetDataLine? = null
    private var thread: Thread? = null

    private val format = AudioFormat(16_000f, 16, 1, true, false)

    /** 返回 null = 已开录；非 null = 给用户看的原因。 */
    fun start(): String? {
        val info = DataLine.Info(TargetDataLine::class.java, format)
        val supported = runCatching { AudioSystem.isLineSupported(info) }.getOrDefault(false)
        if (!supported) return "麦克风不可用：系统没有支持 16 kHz 单声道的输入设备"
        val l = try {
            AudioSystem.getLine(info) as TargetDataLine
        } catch (_: LineUnavailableException) {
            return "麦克风不可用（可能被其他程序占用）"
        } catch (t: Throwable) {
            return "打开麦克风失败：${t.message ?: t.javaClass.simpleName}"
        }
        line = l
        try {
            // 3200 B = 100 ms。给得小一点，停止时最多丢一小块。
            l.open(format, 3200)
            l.start()
        } catch (_: LineUnavailableException) {
            runCatching { l.close() }
            line = null
            return "麦克风不可用（可能被其他程序占用）"
        } catch (t: Throwable) {
            runCatching { l.close() }
            line = null
            return "开始录音失败：${t.message ?: t.javaClass.simpleName}"
        }
        running = true
        thread = Thread({ readLoop(l) }, "opi-speech-audio").apply {
            isDaemon = true
            start()
        }
        return null
    }

    private fun readLoop(l: TargetDataLine) {
        val buf = ByteArray(3200)
        while (running) {
            val n = try {
                l.read(buf, 0, buf.size)
            } catch (t: Throwable) {
                if (running) onError("录音读取失败：${t.message ?: t.javaClass.simpleName}")
                return
            }
            if (!running) return
            if (n > 0) onChunk(buf.copyOf(n))
        }
    }

    fun stop() {
        running = false
        // 先 stop 让阻塞的 read 返回，再 join；顺序反了会白等一个采集缓冲。
        line?.let { runCatching { it.stop() } }
        thread?.let { runCatching { it.join(500) } }
        line?.let { runCatching { it.close() } }
        thread = null
        line = null
    }
}

/**
 * 语音宿主。`toggle` 是从状态栏按钮来的唯一入口：空闲/结束态 = 开始，录音中 = 停止
 * 并识别，准备/识别中 = 取消（浮层的取消按钮也走 [cancel]）。
 */
internal object SpeechHost {
    var ui by mutableStateOf(SpeechUi())
        private set

    @Volatile private var config = SpeechConfig()
    private val commands = LinkedBlockingQueue<SpeechCommand>()
    private val runtime = AsrRuntime()

    init {
        Thread({
            while (true) {
                val cmd = try {
                    commands.poll(50, TimeUnit.MILLISECONDS)
                } catch (_: InterruptedException) {
                    break
                }
                if (cmd != null) runtime.handle(cmd)
                runtime.poll()
            }
        }, "opi-speech").apply {
            isDaemon = true
            start()
        }
    }

    fun setConfig(enabled: Boolean, language: SpeechLanguage) {
        val next = SpeechConfig(enabled, language)
        if (next == config) return
        val wasEnabled = config.enabled
        config = next
        if (wasEnabled && !enabled) commands.offer(SpeechCommand.Cancel)
    }

    fun toggle(commit: (String) -> String?) {
        // 准备/识别中再点是「取消」，不是「开始下一个」—— 否则会排队一个 Toggle，
        // 等当前会话结束后立刻又开一段（用户看到的是点了取消反而还在录）。
        when (ui.phase) {
            SpeechPhase.PREPARING, SpeechPhase.RECOGNIZING -> commands.offer(SpeechCommand.Cancel)
            else -> commands.offer(SpeechCommand.Toggle(commit))
        }
    }

    fun cancel() {
        commands.offer(SpeechCommand.Cancel)
    }

    fun dismiss() {
        commands.offer(SpeechCommand.Dismiss)
    }

    private fun update(next: SpeechUi) {
        ui = next
    }

    private inner class AsrRuntime {
        private var loaded: LoadedAsr? = null
        private var session: Session? = null
        private var recording: AudioCapture? = null

        fun handle(cmd: SpeechCommand) {
            when (cmd) {
                is SpeechCommand.Toggle -> toggle(cmd.commit)
                is SpeechCommand.Audio -> {
                    val s = session ?: return
                    if (ui.phase != SpeechPhase.RECORDING) return
                    feed(s, cmd.bytes)
                }
                is SpeechCommand.AudioError -> {
                    if (session != null) {
                        fail(cmd.message)
                        cancelSession(keepUi = true)
                    }
                }
                SpeechCommand.Cancel -> cancelSession()
                SpeechCommand.Dismiss -> {
                    if (session != null) cancelSession() else update(SpeechUi())
                }
            }
        }

        private fun toggle(commit: (String) -> String?) {
            when (ui.phase) {
                SpeechPhase.PREPARING, SpeechPhase.RECOGNIZING -> cancelSession()
                SpeechPhase.RECORDING -> stop()
                else -> start(commit)
            }
        }

        private fun start(commit: (String) -> String?) {
            val cfg = config
            val engine = cfg.language.engine
            update(SpeechUi(phase = nextSpeechPhase(ui.phase, SpeechTransition.START), engine = engine))
            if (!cfg.enabled) {
                fail("语音输入已关闭：打开 设置 → 语音 启用（默认关闭）")
                return
            }
            val root = speechModelRoot()
            if (!modelReady(engine, root)) {
                fail("未下载 ${engine.title} 模型（约 ${engine.approxMb()} MB）：打开 设置 → 语音 下载")
                return
            }
            val load = loadAsrFfi()
            if (load is AsrLoad.Unavailable) {
                fail(load.reason)
                return
            }
            val ffi = load.ffi
            var h = loaded
            if (h == null || h.engine != engine) {
                h?.let { runCatching { ffi.opi_asr_destroy(it.handle) } }
                loaded = null
                val cfgArg = cStringArg(asrConfigJson(root, cfg.language))
                val ptr = ffi.opi_asr_create(cfgArg)
                if (ptr == null) {
                    fail("创建识别器失败：${ffi.lastError(null)}")
                    return
                }
                if (ffi.opi_asr_load_model(ptr) != 0) {
                    val why = ffi.lastError(ptr)
                    runCatching { ffi.opi_asr_destroy(ptr) }
                    fail("模型加载失败：$why")
                    return
                }
                val max = runCatching { ffi.opi_asr_max_event_bytes() }.getOrDefault(65_536)
                    .coerceIn(4096, 1 shl 20)
                h = LoadedAsr(ffi, engine, ptr, Memory((max + 1).toLong()), max + 1)
                loaded = h
            }
            drain(ffi, h)
            if (ffi.opi_asr_start(h.handle) != 0) {
                fail("启动识别失败：${ffi.lastError(h.handle)}")
                return
            }
            val capture = AudioCapture(
                onChunk = { bytes ->
                    // 队列堆积说明识别侧明显落后；丢新块比无限吃内存好（录音本身不落盘）。
                    if (commands.size < 64) commands.offer(SpeechCommand.Audio(bytes))
                },
                onError = { msg -> commands.offer(SpeechCommand.AudioError(msg)) },
            )
            val audioErr = capture.start()
            if (audioErr != null) {
                runCatching { ffi.opi_asr_cancel(h.handle) }
                drain(ffi, h)
                fail(audioErr)
                return
            }
            recording = capture
            session = Session(engine, commit)
            update(ui.copy(phase = nextSpeechPhase(ui.phase, SpeechTransition.READY), level = 0f, text = "", error = ""))
        }

        private fun stop() {
            val s = session ?: return
            val h = loaded ?: return
            recording?.stop()
            recording = null
            update(ui.copy(phase = nextSpeechPhase(ui.phase, SpeechTransition.STOP), level = 0f, text = ""))
            if (h.ffi.opi_asr_finish(h.handle) != 0) {
                fail("结束录音失败：${h.ffi.lastError(h.handle)}")
                cancelSession(keepUi = true)
                return
            }
            s.deadlineMs = System.currentTimeMillis() + 60_000L
        }

        fun poll() {
            val h = loaded ?: return
            val s = session ?: return
            if (s.deadlineMs > 0 && System.currentTimeMillis() > s.deadlineMs) {
                fail("识别超时（60 秒）：请缩短录音或检查模型")
                cancelSession(keepUi = true)
                return
            }
            repeat(64) {
                val n = h.ffi.opi_asr_poll_event(h.handle, h.eventBuf, h.eventCap)
                when {
                    n == 0 -> return
                    n > 0 -> {
                        val json = h.ffi.readCString(h.eventBuf, n)
                        if (!onEvent(h, s, json)) return
                    }
                    n == -8 -> {
                        fail("事件缓冲不足（${h.eventCap} B）：识别器与桌面绑定版本不一致")
                        cancelSession(keepUi = true)
                        return
                    }
                    else -> {
                        fail("读取识别事件失败（$n）：${h.ffi.lastError(h.handle)}")
                        cancelSession(keepUi = true)
                        return
                    }
                }
            }
        }

        /** true = 继续抽；false = 这一拍停止（会话结束/已出错）。 */
        private fun onEvent(h: LoadedAsr, s: Session, json: String): Boolean {
            return when (eventType(json)) {
                "level" -> {
                    eventLevel(json)?.let { update(ui.copy(level = it)) }
                    true
                }
                "segment" -> {
                    eventText(json)?.takeIf { it.isNotEmpty() }?.let { update(ui.copy(text = it)) }
                    true
                }
                "final" -> {
                    if (ui.phase != SpeechPhase.RECOGNIZING) return true
                    finishFinal(s, eventText(json).orEmpty())
                    false
                }
                "error" -> {
                    val message = eventText(json) ?: "识别器内部错误"
                    fail(if (message.isBlank()) "识别器内部错误" else message)
                    cancelSession(keepUi = true)
                    false
                }
                else -> true
            }
        }

        private fun finishFinal(s: Session, text: String) {
            session = null
            val shown = text.ifBlank { "（未识别到语音）" }
            update(
                ui.copy(
                    phase = nextSpeechPhase(ui.phase, SpeechTransition.FINAL),
                    level = 0f,
                    text = shown,
                    error = "",
                ),
            )
            if (text.isBlank()) return
            val err = try {
                s.commit(text)
            } catch (t: Throwable) {
                "上屏通道异常：${t.message ?: t.javaClass.simpleName}"
            }
            if (err != null) update(ui.copy(error = "上屏失败：$err"))
        }

        private fun feed(s: Session, bytes: ByteArray) {
            val h = loaded ?: return
            var tries = 0
            while (true) {
                val rc = h.ffi.opi_asr_feed_pcm(h.handle, bytes, bytes.size)
                if (rc == 0) return
                // -5 = 队列满：录音是实时的，短暂等它排空；等太久就丢这一块（100 ms）。
                if (rc == -5 && tries < 50) {
                    tries++
                    try {
                        Thread.sleep(10L)
                    } catch (_: InterruptedException) {
                        return
                    }
                    continue
                }
                if (rc == -4 || rc == -5) return
                if (session === s) {
                    fail("音频写入失败（$rc）：${h.ffi.lastError(h.handle)}")
                    cancelSession(keepUi = true)
                }
                return
            }
        }

        /** 清理采集与 Rust 会话。`keepUi` = 保留调用方刚写下的错误浮层。 */
        private fun cancelSession(keepUi: Boolean = false) {
            recording?.stop()
            recording = null
            val h = loaded
            if (h != null && session != null) {
                runCatching { h.ffi.opi_asr_cancel(h.handle) }
                drain(h.ffi, h)
            }
            session = null
            if (!keepUi) update(SpeechUi())
        }

        private fun fail(message: String) {
            update(SpeechUi(phase = nextSpeechPhase(ui.phase, SpeechTransition.FAIL), engine = ui.engine, error = message))
        }

        /** 丢掉句柄里已经排队的事件（取消后/新会话开始前）。 */
        private fun drain(ffi: OpiAsrFfi, h: LoadedAsr) {
            repeat(64) {
                val n = ffi.opi_asr_poll_event(h.handle, h.eventBuf, h.eventCap)
                if (n <= 0) return
            }
        }
    }
}

private val EVENT_TYPE = Regex("\"type\"\\s*:\\s*\"([a-z_]+)\"")
private val RMS_DB = Regex("\"rms_db\"\\s*:\\s*(-?[0-9]+(?:\\.[0-9]+)?)")

internal fun eventType(json: String): String? =
    EVENT_TYPE.find(json)?.groupValues?.getOrNull(1)

/** rms_db → 0..1；-60 dB 以下按 0 画。 */
internal fun eventLevel(json: String): Float? {
    val db = RMS_DB.find(json)?.groupValues?.getOrNull(1)?.toFloatOrNull() ?: return null
    return ((db + 60f) / 60f).coerceIn(0f, 1f)
}

/** final/segment/error 的 `text`（`message` 也认，兼容 error 事件）。 */
internal fun eventText(json: String): String? {
    val obj = parseLine(json) ?: return null
    return (obj["text"] as? JVal.JStr)?.v ?: (obj["message"] as? JVal.JStr)?.v
}
