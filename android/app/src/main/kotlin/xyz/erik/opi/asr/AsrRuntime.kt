// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import xyz.erik.opi.settings.SettingsActivity
import org.json.JSONObject
import java.util.concurrent.Executors

/** 键盘上的话筒入口；[available] = 原生库可加载（32 位 ABI 不打包 → 隐藏）。 */
internal data class AsrMic(val available: Boolean, val start: () -> Unit)

/** 六个空格键共用的入口。null（预览/单测/32 位设备）= 不显示话筒。 */
internal val LocalAsrMic = compositionLocalOf<AsrMic?> { null }

/**
 * 语音输入运行时：长按 → 权限/模型门 → 模型装载 → 录音 → 识别 → 上屏。
 *
 * 所有 native 调用与 AudioRecord 都在**单线程** [worker] 上，主线程只改
 * [request] / [uiState]；native 句柄因此不需要额外的跨线程约定。主线程换编辑器/
 * 收起键盘/销毁时 [onEditorChanged] 作废世代，迟到的 final 不会写进新输入框。
 *
 * 模型首次装载会阻塞工作线程（239MB int8 读盘 + ONNX 初始化），浮层保持
 * PREPARING；装载好的句柄跨会话缓存，停止录音不卸载。
 */
internal object AsrRuntime {
    private const val TAG = AsrNative.TAG
    private const val SAMPLE_RATE = 16_000
    private const val CHUNK_BYTES = 3_200 // 100ms PCM16 mono
    private const val FEED_RETRY_MS = 45_000L
    private const val FINISH_TIMEOUT_MS = 120_000L
    private const val POLL_MS = 30L
    private const val FALLBACK_EVENT_BYTES = 64 * 1024

    private enum class Request { IDLE, RUN, STOP, CANCEL }

    private class AsrFailure(message: String) : Exception(message)

    private sealed interface Polled {
        data object None : Polled
        data class Final(val text: String) : Polled
        data class Error(val message: String) : Polled
    }

    private val main = Handler(Looper.getMainLooper())
    private val worker = Executors.newSingleThreadExecutor { r ->
        Thread(r, "opi-asr").apply { isDaemon = true }
    }
    private val generation = AsrGeneration()

    /** 只在主线程读写。 */
    private var appContext: Context? = null
    private var whisperPref: (() -> Boolean)? = null
    private var commit: ((String) -> Unit)? = null

    @Volatile
    private var request = Request.IDLE

    /** 只在 [worker] 上读写。 */
    private var handle = 0L
    private var loadedKey = ""
    private var eventBuf: ByteArray? = null

    var uiState by mutableStateOf(AsrUiState())
        private set

    /** 服务创建输入视图时接线；重复调用只换回调，不打断进行中的会话。 */
    fun attach(context: Context, whisper: () -> Boolean, commit: (String) -> Unit) {
        appContext = context.applicationContext
        whisperPref = whisper
        this.commit = commit
    }

    fun start() {
        val context = appContext ?: return
        if (!AsrNative.loaded) return
        val whisper = whisperPref?.invoke() ?: false
        val gate = asrGate(
            available = true,
            permissionGranted = micGranted(context),
            modelReady = AsrModels.modelReady(context, whisper),
        )
        when (gate) {
            AsrGate.UNSUPPORTED -> return
            AsrGate.NEED_PERMISSION -> {
                uiState = AsrUiState(
                    AsrPhase.PERMISSION_DENIED,
                    "需要麦克风权限才能语音输入",
                )
                AsrPermissionActivity.start(context)
                return
            }
            AsrGate.MODEL_MISSING -> {
                uiState = AsrUiState(AsrPhase.MODEL_MISSING, whisper = whisper)
                return
            }
            AsrGate.READY -> Unit
        }
        val gen = generation.begin()
        request = Request.RUN
        uiState = AsrUiState(AsrPhase.PREPARING, "正在准备本地语音模型…")
        worker.execute { runSession(context, gen, whisper) }
    }

    /** 锁定录音 → 停止收音并等待 final。只对进行中的会话生效。 */
    fun stop() {
        if (uiState.phase != AsrPhase.PREPARING && uiState.phase != AsrPhase.LISTENING) return
        request = Request.STOP
        uiState = reduce(uiState, AsrEvent.Stop)
    }

    /** 取消（浮层按钮/换编辑器/收起键盘/销毁）：作废世代，迟到结果直接丢弃。 */
    fun onEditorChanged() {
        request = Request.CANCEL
        generation.invalidate()
        uiState = AsrUiState()
    }

    fun cancel() = onEditorChanged()

    fun detach() {
        onEditorChanged()
        appContext = null
        whisperPref = null
        commit = null
        // 排在进行中的会话之后：录音线程先退出，再卸载模型（约 240–360MB）。
        worker.execute { unloadModel() }
    }

    /** 权限 Activity 的结果。授予后不自动开录：用户再长按一次，避开生命周期竞态。 */
    fun onPermissionResult(granted: Boolean) {
        if (uiState.phase != AsrPhase.PERMISSION_DENIED) return
        uiState = reduce(uiState, if (granted) AsrEvent.Cancel else AsrEvent.Denied)
    }

    fun openSettings() {
        val context = appContext ?: return
        onEditorChanged()
        context.startActivity(
            Intent(context, SettingsActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
        )
    }

    fun requestPermission() {
        appContext?.let { AsrPermissionActivity.start(it) }
    }

    private fun micGranted(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.M ||
            context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED

    private fun runSession(context: Context, gen: Int, whisper: Boolean) {
        try {
            if (!generation.isCurrent(gen)) return
            if (!ensureModel(context, gen, whisper)) return
            if (!generation.isCurrent(gen)) return
            if (request != Request.RUN) {
                post(gen, AsrEvent.Cancel)
                return
            }
            record(gen)
        } catch (t: Throwable) {
            Log.w(TAG, "asr session failed", t)
            if (t is SecurityException) {
                post(gen, AsrEvent.Denied)
            } else {
                post(gen, AsrEvent.Fail(t.message ?: "语音识别失败"))
            }
        } finally {
            // 每个会话结束都回到 native 空闲态；final 已取出，清事件无副作用。
            if (handle != 0L) AsrNative.cancel(handle)
        }
    }

    /** 返回 false = 已给出 MODEL_MISSING/ERROR 浮层，会话不再继续。 */
    private fun ensureModel(context: Context, gen: Int, whisper: Boolean): Boolean {
        val pack = AsrModels.selected(context, whisper)
        if (!AsrModels.packReady(context, pack)) {
            post(gen, AsrEvent.Missing(whisper))
            return false
        }
        val key = "${pack.dir}|$whisper"
        if (handle != 0L && loadedKey != key) unloadModel()
        if (handle != 0L) return true

        val config = JSONObject()
            .put("models_dir", AsrModels.root(context).absolutePath)
            // Whisper 固定 en；默认包用 auto 让 SenseVoice 自己判中/英/粤。
            .put("language", if (whisper) "en" else "auto")
            // 下载时已逐文件校验 SHA-256；这里只让加载器查大小，避免每次长按
            // 对 239/358MB 重算哈希卡住 PREPARING。
            .put("verify_models", false)
            .put("num_threads", 2)
            .toString()
        val created = AsrNative.create(config)
        if (created == 0L) {
            val why = lastError(0L)
            post(gen, AsrEvent.Fail(why.ifEmpty { "语音模型初始化失败" }))
            return false
        }
        val rc = AsrNative.loadModel(created)
        if (rc != 0) {
            val why = lastError(created)
            AsrNative.destroy(created)
            if (rc == AsrNative.ERR_MODEL) {
                post(gen, AsrEvent.Missing(whisper))
            } else {
                post(gen, AsrEvent.Fail(why.ifEmpty { "模型加载失败（$rc）" }))
            }
            return false
        }
        handle = created
        loadedKey = key
        return true
    }

    private fun record(gen: Int) {
        val record = openRecorder()
        var started = false
        try {
            record.startRecording()
            started = true
            val rc = AsrNative.start(handle)
            if (rc != 0) throw AsrFailure(nativeMessage(rc))
            post(gen, AsrEvent.Ready)

            val pcm = ByteArray(CHUNK_BYTES)
            while (request == Request.RUN && generation.isCurrent(gen)) {
                val n = record.read(pcm, 0, pcm.size)
                if (n < 0) throw AsrFailure("麦克风读取失败（$n）")
                if (n > 0) feed(pcm, n, gen)
                when (val e = drain(gen)) {
                    is Polled.Error -> throw AsrFailure(e.message)
                    else -> Unit
                }
            }
            if (!generation.isCurrent(gen) || request == Request.CANCEL) return
            if (request == Request.STOP) transcribe(gen)
        } finally {
            if (started) runCatching { record.stop() }
            record.release()
        }
    }

    /** native 队列 32 块；Whisper 解码慢时回压，不丢音频（读麦克风暂时停住）。 */
    private fun feed(pcm: ByteArray, n: Int, gen: Int) {
        val bytes = if (n == pcm.size) pcm else pcm.copyOf(n)
        val deadline = SystemClock.elapsedRealtime() + FEED_RETRY_MS
        while (true) {
            val rc = AsrNative.feedPcm(handle, bytes)
            if (rc == 0) return
            if (rc != AsrNative.ERR_BUSY) throw AsrFailure(nativeMessage(rc))
            if (request != Request.RUN || !generation.isCurrent(gen)) return
            if (SystemClock.elapsedRealtime() > deadline) {
                throw AsrFailure("音频处理跟不上，请缩短录音")
            }
            Thread.sleep(20)
        }
    }

    private fun transcribe(gen: Int) {
        val rc = AsrNative.finish(handle)
        if (rc != 0) throw AsrFailure(nativeMessage(rc))
        val deadline = SystemClock.elapsedRealtime() + FINISH_TIMEOUT_MS
        while (generation.isCurrent(gen) && request != Request.CANCEL) {
            when (val e = drain(gen)) {
                is Polled.Final -> {
                    complete(gen, e.text)
                    return
                }
                is Polled.Error -> throw AsrFailure(e.message)
                Polled.None -> Unit
            }
            if (SystemClock.elapsedRealtime() > deadline) {
                throw AsrFailure("识别超时，请重试")
            }
            Thread.sleep(POLL_MS)
        }
    }

    private fun complete(gen: Int, raw: String) {
        val text = asrCommitText(raw)
        if (text == null) {
            post(gen, AsrEvent.Fail("没有识别到语音，请重试"))
            return
        }
        main.post {
            if (!generation.isCurrent(gen)) return@post
            runCatching { commit?.invoke(text) }
                .onFailure { Log.w(TAG, "commit failed", it) }
            uiState = reduce(uiState, AsrEvent.Final(true))
        }
    }

    private fun drain(gen: Int): Polled {
        val buf = eventBuf ?: ByteArray(
            AsrNative.maxEventBytes().coerceAtLeast(FALLBACK_EVENT_BYTES),
        ).also { eventBuf = it }
        while (true) {
            val n = AsrNative.pollEvent(handle, buf)
            if (n == 0) return Polled.None
            if (n < 0) return Polled.Error("事件读取失败（$n）")
            val event = try {
                JSONObject(String(buf, 0, n, Charsets.UTF_8))
            } catch (e: Exception) {
                return Polled.Error("事件解析失败")
            }
            when (event.optString("type")) {
                "level" -> post(gen, AsrEvent.Level(event.optDouble("rms_db", -60.0).toFloat()))
                "final" -> return Polled.Final(event.optString("text"))
                "error" -> return Polled.Error(
                    event.optString("message").ifEmpty { event.optString("code") },
                )
                else -> Unit // segment 只用于候选预览；最终文本走 final。
            }
        }
    }

    @Suppress("DEPRECATION")
    private fun openRecorder(): AudioRecord {
        val min = AudioRecord.getMinBufferSize(
            SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
        )
        if (min <= 0) throw AsrFailure("麦克风不可用")
        val record = AudioRecord(
            MediaRecorder.AudioSource.VOICE_RECOGNITION,
            SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
            maxOf(min, CHUNK_BYTES * 8),
        )
        if (record.state != AudioRecord.STATE_INITIALIZED) {
            record.release()
            throw AsrFailure("麦克风初始化失败")
        }
        return record
    }

    private fun post(gen: Int, event: AsrEvent) {
        main.post {
            if (!generation.isCurrent(gen)) return@post
            uiState = reduce(uiState, event)
        }
    }

    private fun nativeMessage(rc: Int): String =
        lastError(handle).ifEmpty { "语音识别失败（$rc）" }

    private fun lastError(handle: Long): String {
        val buf = eventBuf ?: ByteArray(FALLBACK_EVENT_BYTES).also { eventBuf = it }
        val n = AsrNative.lastError(handle, buf)
        return if (n > 0) String(buf, 0, n, Charsets.UTF_8) else ""
    }

    private fun unloadModel() {
        if (handle != 0L) {
            AsrNative.destroy(handle)
            handle = 0L
            loadedKey = ""
        }
    }
}
