// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Java Sound 采集（从 Speech.kt 拆出，500 行门禁）：16 kHz / 16 bit / mono /
//! little-endian，专用读线程；音频只在本机内存中经过，不落盘、不写日志。

package xyz.erik.opi.candidate

import javax.sound.sampled.AudioFormat
import javax.sound.sampled.AudioSystem
import javax.sound.sampled.DataLine
import javax.sound.sampled.LineUnavailableException
import javax.sound.sampled.TargetDataLine

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
