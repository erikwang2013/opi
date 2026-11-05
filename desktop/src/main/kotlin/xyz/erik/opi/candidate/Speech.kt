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
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

private sealed interface SpeechCommand {
    // 开始/停止在入队时就定死：外层先把 ui.phase 置成 PREPARING，若 worker 再按
    // phase 判断「现在是不是该取消」，会把自己刚置的准备态当成第二次点击而立刻取消。
    data class Start(val commit: (String) -> String?) : SpeechCommand
    data object Stop : SpeechCommand
    data class Audio(val bytes: ByteArray) : SpeechCommand
    data class AudioError(val message: String) : SpeechCommand
    data object Cancel : SpeechCommand
    data object Dismiss : SpeechCommand
    data object Warm : SpeechCommand
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


/**
 * 语音宿主。`toggle` 是从状态栏按钮来的唯一入口：空闲/结束态 = 开始，录音中 = 停止
 * 并识别，准备/识别中 = 取消（浮层的取消按钮也走 [cancel]）。
 */
internal class SpeechHostImpl {
    var ui by mutableStateOf(SpeechUi())
        private set

    @Volatile private var config = SpeechConfig()
    /** 取消/关闭立刻隐藏 UI；加载线程回来时用代际校验丢弃过期状态。 */
    @Volatile private var epoch = 0
    private val commands = LinkedBlockingQueue<SpeechCommand>()
    private val runtime = AsrRuntime()

    /** 最终上屏失败时留给「关闭浮层」的最后一次重试：浮层消失后焦点回到输入框。 */
    private data class PendingRetry(val text: String, val commit: (String) -> String?)
    @Volatile private var pendingRetry: PendingRetry? = null

    init {
        Thread({
            while (true) {
                val cmd = try {
                    commands.poll(50, TimeUnit.MILLISECONDS)
                } catch (_: InterruptedException) {
                    break
                }
                // 工作线程绝不能因一条命令的异常退出：线程一死，后续 Toggle 只会
                // 把浮层停在 PREPARING，用户看到的就是「一直正在加载模型」。
                if (cmd != null) {
                    try {
                        runtime.handle(cmd)
                    } catch (t: Throwable) {
                        runtime.crashed(t)
                    }
                }
                try {
                    runtime.poll()
                } catch (t: Throwable) {
                    runtime.crashed(t)
                }
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
        val languageChanged = next.language != config.language
        config = next
        if (wasEnabled && (!enabled || languageChanged)) commands.offer(SpeechCommand.Cancel)
        // 开启语音/切换引擎时预加载：第一次点话筒不再现场读 239/358MB 模型。
        if (enabled && (!wasEnabled || languageChanged)) commands.offer(SpeechCommand.Warm)
    }

    /** 后台预热当前输入模式对应的引擎；失败不打扰 UI，点话筒时再报。 */
    fun warm() {
        if (config.enabled) commands.offer(SpeechCommand.Warm)
    }

    fun toggle(commit: (String) -> String?) {
        // 准备/识别中再点是「取消」，不是「开始下一个」—— 否则会排队一个 Toggle，
        // 等当前会话结束后立刻又开一段（用户看到的是点了取消反而还在录）。
        when (ui.phase) {
            SpeechPhase.PREPARING, SpeechPhase.RECOGNIZING -> {
                epoch++
                update(SpeechUi())
                commands.offer(SpeechCommand.Cancel)
            }
            SpeechPhase.RECORDING -> commands.offer(SpeechCommand.Stop)
            else -> {
                // 先让浮层在同一次点击里出现；模型/麦克风都在工作线程上再准备。
                if (config.enabled) update(SpeechUi(phase = SpeechPhase.PREPARING, engine = config.language.engine))
                commands.offer(SpeechCommand.Start(commit))
            }
        }
    }

    /** 浮层里的「停止」：只对正在录音的会话生效。 */
    fun stop() {
        if (ui.phase == SpeechPhase.RECORDING) commands.offer(SpeechCommand.Stop)
    }

    fun cancel() {
        epoch++
        update(SpeechUi())
        commands.offer(SpeechCommand.Cancel)
    }

    fun dismiss() {
        epoch++
        update(SpeechUi())
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
                is SpeechCommand.Start -> start(cmd.commit)
                SpeechCommand.Stop -> stop()
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
                SpeechCommand.Warm -> ensureEngine(config.language.engine, updateErrors = false, startEpoch = epoch)
                SpeechCommand.Dismiss -> {
                    val retry = pendingRetry
                    pendingRetry = null
                    if (session != null) cancelSession() else update(SpeechUi())
                    if (retry != null) {
                        // 浮层已隐藏，等焦点回到输入框再补一次；失败也不再弹窗（用户已关闭）。
                        try {
                            Thread.sleep(150L)
                        } catch (_: InterruptedException) {
                            return
                        }
                        runCatching { retry.commit(retry.text) }
                    }
                }
            }
        }


        /** 加载/复用引擎；warm 不触发完整包解压，避免启动时先把语音线程堵死。 */
        private fun ensureEngine(engine: SpeechEngine, updateErrors: Boolean, startEpoch: Int = epoch): LoadedAsr? {
            val root = speechModelRoot()
            // 先查引擎再查模型：普通/基础包最容易先下 239 MB 模型、最后才发现引擎
            // 还没下，提示还只说“模型未下载”。引擎是前置依赖，错误顺序要一致。
            if (!SpeechEnginePack.bundled() && !SpeechEnginePack.ready()) {
                if (updateErrors) {
                    fail(
                        if (SpeechEnginePack.supported()) {
                            "语音引擎未下载（约 ${SpeechEnginePack.approxMb()} MB）：" +
                                "打开 设置 → 语音 先下载语音引擎，再下载模型"
                        } else {
                            "当前平台没有可用的语音引擎组件"
                        },
                    )
                }
                return null
            }
            if (!modelReady(engine, root)) {
                if (!updateErrors) return null
                if (!bundledJar()) {
                    fail("未下载 ${engine.title} 模型（约 ${engine.approxMb()} MB）：打开 设置 → 语音 下载")
                    return null
                }
                val prep = ensureBundledModels(
                    root, engine,
                    onProgress = { done, total ->
                        val pct = if (total <= 0) 0 else (done * 100 / total).toInt().coerceIn(0, 100)
                        update(ui.copy(text = "正在准备内置模型… $pct%"))
                    },
                    isCancelled = { epoch != startEpoch },
                )
                when (prep) {
                    ModelPrep.Cancelled -> return null
                    is ModelPrep.Failed -> {
                        fail(prep.message)
                        return null
                    }
                    ModelPrep.Ready -> Unit
                }
                update(ui.copy(text = "正在加载模型…"))
            }
            // 文件齐全但清单缺失（旧版下载/解压）⇒ 补写；Rust 加载器无条件读它。
            ensureModelManifest(engine, root)
            val load = loadAsrFfi()
            val ffi = when (load) {
                is AsrLoad.Loaded -> load.ffi
                is AsrLoad.Unavailable -> {
                    if (updateErrors) fail(load.reason)
                    return null
                }
            }
            val existing = loaded
            if (existing != null && existing.engine == engine) return existing
            existing?.let { runCatching { ffi.opi_asr_destroy(it.handle) } }
            loaded = null
            val cfgArg = cStringArg(asrConfigJson(root, config.language))
            val ptr = ffi.opi_asr_create(cfgArg)
            if (ptr == null) {
                if (updateErrors) fail("创建识别器失败：${ffi.lastError(null)}")
                return null
            }
            if (ffi.opi_asr_load_model(ptr) != 0) {
                val why = ffi.lastError(ptr)
                runCatching { ffi.opi_asr_destroy(ptr) }
                if (updateErrors) fail("模型加载失败：$why")
                return null
            }
            val max = runCatching { ffi.opi_asr_max_event_bytes() }.getOrDefault(65_536)
                .coerceIn(4096, 1 shl 20)
            val made = LoadedAsr(ffi, engine, ptr, Memory((max + 1).toLong()), max + 1)
            loaded = made
            return made
        }

        private fun start(commit: (String) -> String?) {
            // 上一次最终上屏失败的文字：下次开始前再试一次（用户没点“关闭”而是直接重录）。
            pendingRetry?.let {
                pendingRetry = null
                runCatching { it.commit(it.text) }
            }
            val myEpoch = epoch
            val cfg = config
            val engine = cfg.language.engine
            update(SpeechUi(phase = nextSpeechPhase(ui.phase, SpeechTransition.START), engine = engine))
            if (!cfg.enabled) {
                fail("语音输入已关闭：打开 设置 → 语音 打开总开关")
                return
            }
            val h = ensureEngine(engine, updateErrors = true, startEpoch = myEpoch) ?: return
            val ffi = h.ffi
            if (myEpoch != epoch) return
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
            // 模型加载完不等于能录音：麦克风被占用/驱动卡住时，别让浮层继续写
            // “正在加载模型”误导用户。真正的失败仍由 capture.start() 的返回值报。
            update(ui.copy(text = "正在打开麦克风…"))
            val audioErr = capture.start()
            if (audioErr != null) {
                runCatching { ffi.opi_asr_cancel(h.handle) }
                drain(ffi, h)
                fail(audioErr)
                return
            }
            if (myEpoch != epoch) {
                capture.stop()
                runCatching { ffi.opi_asr_cancel(h.handle) }
                return
            }
            recording = capture
            OtherAudio.pause()
            session = Session(engine, commit, myEpoch)
            update(ui.copy(phase = nextSpeechPhase(ui.phase, SpeechTransition.READY), level = 0f, text = "", error = ""))
        }

        private fun stop() {
            val s = session ?: return
            val h = loaded ?: return
            recording?.stop()
            recording = null
            OtherAudio.resume()
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
            if (s.epoch != epoch) return
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
                    val text = eventText(json).orEmpty().trim()
                    if (text.isNotEmpty()) {
                        s.segmentText += text
                        update(ui.copy(text = s.segmentText))
                        val err = try {
                            s.commit(text)
                        } catch (t: Throwable) {
                            "上屏通道异常：${t.message ?: t.javaClass.simpleName}"
                        }
                        if (err == null) {
                            s.segmentsCommitted++
                        } else {
                            // 这段没进文档：记下来，停止时再整体试一次。
                            s.failedText += text
                            update(ui.copy(error = "上屏失败：$err"))
                        }
                    }
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
            // 有段没上屏：停止时整体重试这些段（浮层即将/已经离开焦点，落字成功率更高）。
            // 全部段都成功过才跳过 final，避免把已上屏的内容再提交一遍。
            val retry = speechRetryText(s.failedText, s.segmentsCommitted, text)
            if (retry.isBlank()) return
            val err = try {
                s.commit(retry)
            } catch (t: Throwable) {
                "上屏通道异常：${t.message ?: t.javaClass.simpleName}"
            }
            if (err != null) {
                pendingRetry = PendingRetry(retry, s.commit)
                update(ui.copy(error = "上屏失败：$err（关闭浮层时会再试一次）"))
            } else {
                pendingRetry = null
            }
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
            OtherAudio.resume()
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

        /** 工作线程兜底：把异常折成浮层错误，线程继续处理后续命令。 */
        fun crashed(t: Throwable) {
            val active = ui.phase != SpeechPhase.IDLE
            runCatching { cancelSession() }
            // 启动预热（Warm）失败且用户没点话筒：静默保住线程，别在登录时弹错误浮层。
            if (active) fail("语音内部错误：${t.message ?: t.javaClass.simpleName}")
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
internal val SpeechHost = SpeechHostImpl()
