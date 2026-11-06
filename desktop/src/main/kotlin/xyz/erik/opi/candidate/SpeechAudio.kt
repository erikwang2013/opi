// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Java Sound 采集（从 Speech.kt 拆出，500 行门禁）：最终输出 16 kHz / 16 bit /
//! mono / little-endian，专用读线程；音频只在本机内存中经过，不落盘、不写日志。
//!
//! Windows 的 Java Sound 经常只暴露 44.1/48 kHz 立体声，直接要 16 kHz 单声道会
//! `isLineSupported=false`（现场「语音没反应」的一类根因）——这里按候选格式逐个
//! 尝试，读到的 PCM 再线性重采样成 16 kHz 单声道。

package xyz.erik.opi.candidate

import javax.sound.sampled.AudioFormat
import javax.sound.sampled.AudioSystem
import javax.sound.sampled.DataLine
import javax.sound.sampled.TargetDataLine

/** Java Sound 采集：先取设备支持的格式，再转成 16 kHz / 16 bit / mono。 */
internal class AudioCapture(
    private val onChunk: (ByteArray) -> Unit,
    private val onError: (String) -> Unit,
) {
    @Volatile private var running = false
    @Volatile private var line: TargetDataLine? = null
    @Volatile private var thread: Thread? = null
    @Volatile private var monitor: Thread? = null
    /** 每次重开采集线都换代，旧读线程看到代数变了就退出，避免两条线同时喂 PCM。 */
    @Volatile private var generation = 0
    /** 最近一次读到 PCM 的时刻；超过 STALL_MS 没数据就判定采集线卡死并重开。 */
    @Volatile private var lastDataMs = 0L
    private var sourceRate = 16_000
    private var sourceChannels = 1

    private companion object {
        /** PipeWire/Pulse 偶发挂起采集线：静音时也应持续读到 0 样本，长时间无数据即重开。 */
        const val STALL_MS = 4_000L
    }

    private val candidates = listOf(
        AudioFormat(16_000f, 16, 1, true, false),
        AudioFormat(44_100f, 16, 1, true, false),
        AudioFormat(48_000f, 16, 1, true, false),
        AudioFormat(44_100f, 16, 2, true, false),
        AudioFormat(48_000f, 16, 2, true, false),
        AudioFormat(8_000f, 16, 1, true, false),
    )

    /** 返回 null = 已开录；非 null = 给用户看的原因。 */
    @Synchronized
    fun start(): String? {
        val err = openLine()
        if (err != null) return err
        startMonitor()
        return null
    }

    /** 按候选格式开一条采集线并起读线程；调用方持锁。 */
    private fun openLine(): String? {
        val failures = ArrayList<String>(2)
        for (format in candidates) {
            val info = DataLine.Info(TargetDataLine::class.java, format)
            if (!runCatching { AudioSystem.isLineSupported(info) }.getOrDefault(false)) continue
            val l = try {
                AudioSystem.getLine(info) as TargetDataLine
            } catch (t: Throwable) {
                failures += "${format.sampleRate.toInt()}Hz/${format.channels}ch: ${t.javaClass.simpleName}"
                continue
            }
            try {
                // 100 ms 量级；停止时最多丢一小块。
                l.open(format, (format.sampleRate.toInt() / 10) * format.channels * 2)
                l.start()
            } catch (t: Throwable) {
                runCatching { l.close() }
                failures += "${format.sampleRate.toInt()}Hz/${format.channels}ch: ${t.javaClass.simpleName}"
                continue
            }
            val gen = ++generation
            line = l
            sourceRate = format.sampleRate.toInt()
            sourceChannels = format.channels
            lastDataMs = System.currentTimeMillis()
            running = true
            thread = Thread({ readLoop(l, gen) }, "opi-speech-audio").apply {
                isDaemon = true
                start()
            }
            return null
        }
        return if (failures.isEmpty()) {
            "麦克风不可用：系统没有可录音的输入设备（需要 16/44.1/48 kHz、16 bit）"
        } else {
            "麦克风不可用（可能被其他程序占用）：${failures.joinToString("；")}"
        }
    }

    /** 采集线超过 4 秒没有任何 PCM（不是静音，是读调用不再返回）就换一条新线。 */
    private fun startMonitor() {
        if (monitor?.isAlive == true) return
        monitor = Thread({
            while (running) {
                try {
                    Thread.sleep(500L)
                } catch (_: InterruptedException) {
                    return@Thread
                }
                if (running && System.currentTimeMillis() - lastDataMs > STALL_MS) {
                    restartStalledLine()
                }
            }
        }, "opi-speech-audio-watch").apply {
            isDaemon = true
            start()
        }
    }

    @Synchronized
    private fun restartStalledLine() {
        if (!running) return
        generation++ // 旧读线程看到代数变化后自行退出
        val old = line
        line = null
        runCatching { old?.stop() }
        runCatching { old?.close() }
        thread?.let { runCatching { it.join(300) } }
        thread = null
        val err = openLine()
        if (err != null) {
            running = false
            onError(err)
        }
    }

    private fun readLoop(l: TargetDataLine, gen: Int) {
        val buf = ByteArray(9_600)
        while (running && generation == gen) {
            val n = try {
                l.read(buf, 0, buf.size)
            } catch (t: Throwable) {
                if (running && generation == gen) {
                    onError("录音读取失败：${t.message ?: t.javaClass.simpleName}")
                }
                return
            }
            if (!running || generation != gen) return
            if (n > 0) {
                lastDataMs = System.currentTimeMillis()
                val pcm = to16kMono(buf, n)
                if (pcm.isNotEmpty()) onChunk(pcm)
            }
        }
    }

    /** 任意受支持格式 → 16 kHz 单声道 16-bit LE。线性插值足够给 ASR 用。 */
    private fun to16kMono(src: ByteArray, len: Int): ByteArray =
        pcm16To16kMono(src, len, sourceRate, sourceChannels)

    @Synchronized
    fun stop() {
        running = false
        generation++
        // 先 stop 让阻塞的 read 返回，再 join；顺序反了会白等一个采集缓冲。
        line?.let { runCatching { it.stop() } }
        thread?.let { runCatching { it.join(500) } }
        line?.let { runCatching { it.close() } }
        monitor?.let { runCatching { it.interrupt() } }
        thread = null
        line = null
        monitor = null
    }
}

/** 16-bit PCM → 16 kHz 单声道 16-bit LE；纯函数，单测钉住重采样与声道混合。 */
internal fun pcm16To16kMono(
    src: ByteArray,
    len: Int,
    sourceRate: Int,
    sourceChannels: Int,
): ByteArray {
    val frameBytes = sourceChannels * 2
    val frames = len / frameBytes
    if (frames <= 0) return ByteArray(0)
    if (sourceRate == 16_000 && sourceChannels == 1) return src.copyOf(frames * 2)
    val outFrames = (frames.toDouble() * 16_000.0 / sourceRate).toInt().coerceAtLeast(1)
    val out = ByteArray(outFrames * 2)
    fun sampleAt(frame: Int): Int {
        val base = frame * frameBytes
        fun le16(offset: Int): Int =
            (((src[offset].toInt() and 0xff) or (src[offset + 1].toInt() shl 8)).toShort()).toInt()
        if (sourceChannels == 1) return le16(base)
        return (le16(base) + le16(base + 2)) / 2
    }
    for (i in 0 until outFrames) {
        val pos = i * sourceRate / 16_000.0
        val i0 = pos.toInt().coerceAtMost(frames - 1)
        val i1 = (i0 + 1).coerceAtMost(frames - 1)
        val frac = pos - i0
        var sample = sampleAt(i0)
        if (i1 != i0) sample = (sample + (sampleAt(i1) - sample) * frac).toInt()
        val v = sample.coerceIn(-32768, 32767)
        out[i * 2] = (v and 0xff).toByte()
        out[i * 2 + 1] = ((v shr 8) and 0xff).toByte()
    }
    return out
}
