// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean
import java.util.zip.ZipInputStream

/**
 * 普通包/基础包的语音引擎下载（2026-11-01 用户裁决：引擎只随完整包）。
 *
 * 组件地址、大小、SHA-256 都来自官网 `version.json`；下载后先整包校验，再只解压
 * [AsrEngine.LIBS] 四个库到 `filesDir/asr/engine/<abi>/`，最后尝试动态加载。
 */
internal object AsrEngineDownloader {
    private const val TAG = "OpiAsr"

    sealed interface State {
        data object Idle : State
        data class Running(val received: Long, val total: Long) : State
        data object Done : State
        data class Failed(val message: String) : State
    }

    private val main = Handler(Looper.getMainLooper())
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val running = AtomicBoolean(false)

    @Volatile
    private var lastProgressAt = 0L

    var state by mutableStateOf<State>(State.Idle)
        private set

    fun download(context: Context) {
        if (!running.compareAndSet(false, true)) return
        val app = context.applicationContext
        if (AsrEngine.ready(app)) {
            state = State.Done
            running.set(false)
            return
        }
        state = State.Running(0, AsrEngine.APPROX_BYTES)
        scope.launch {
            try {
                install(app)
                main.post {
                    state = State.Done
                    running.set(false)
                }
            } catch (t: Throwable) {
                Log.w(TAG, "ASR engine download failed", t)
                main.post {
                    state = State.Failed(t.message ?: "下载失败")
                    running.set(false)
                }
            }
        }
    }

    private fun install(context: Context) {
        val remote = AsrEngine.remoteSpec().getOrElse {
            throw IOException("无法获取语音引擎清单：${it.message ?: "网络不可用"}")
        }
        state = State.Running(0, remote.size)
        val zip = File(context.cacheDir, "opi-asr-engine-${AsrEngine.abi()}.zip")
        try {
            try {
                downloadFile(remote.url, zip, remote.size, remote.sha256)
            } catch (t: Throwable) {
                downloadFile(remote.fallback, zip, remote.size, remote.sha256)
            }
            extract(context, zip)
        } finally {
            zip.delete()
        }
        AsrNative.reload(context)
        main.post { state = State.Running(remote.size, remote.size) }
    }

    private fun downloadFile(url: String, dest: File, expectedBytes: Long, expectedSha256: String) {
        dest.parentFile?.mkdirs()
        val part = File(dest.parentFile, dest.name + ".part")
        part.delete()
        val conn = (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 30_000
            readTimeout = 60_000
            instanceFollowRedirects = true
            useCaches = false
            setRequestProperty("User-Agent", "OPI-Android")
        }
        try {
            val code = conn.responseCode
            if (code !in 200..299) throw IOException("HTTP $code")
            val declared = conn.contentLength
            if (declared > 0 && declared.toLong() != expectedBytes) {
                throw IOException("文件大小不符：服务器 $declared，应 $expectedBytes")
            }
            val digest = MessageDigest.getInstance("SHA-256")
            var received = 0L
            conn.inputStream.use { input ->
                part.outputStream().buffered().use { output ->
                    val buf = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        digest.update(buf, 0, n)
                        output.write(buf, 0, n)
                        received += n
                        if (received > expectedBytes) throw IOException("文件超长")
                        publish(received, expectedBytes)
                    }
                }
            }
            if (received != expectedBytes) {
                throw IOException("文件大小不符：下载 $received，应 $expectedBytes")
            }
            val got = digest.digest().joinToString("") { "%02x".format(it.toInt() and 0xff) }
            if (!got.equals(expectedSha256, ignoreCase = true)) {
                throw IOException("SHA-256 校验失败")
            }
            if (!part.renameTo(dest)) {
                dest.delete()
                if (!part.renameTo(dest)) {
                    part.copyTo(dest, overwrite = true)
                    part.delete()
                }
            }
        } finally {
            conn.disconnect()
            if (part.exists()) part.delete()
        }
    }

    private fun extract(context: Context, zip: File) {
        val dest = AsrEngine.root(context) ?: throw IOException("当前设备 ABI 不支持语音引擎")
        val tmp = File(dest.parentFile, dest.name + ".tmp")
        tmp.deleteRecursively()
        tmp.mkdirs()
        val allowed = AsrEngine.LIBS.toSet()
        val found = mutableSetOf<String>()
        ZipInputStream(zip.inputStream().buffered()).use { zin ->
            while (true) {
                val entry = zin.nextEntry ?: break
                val name = entry.name.substringAfterLast('/')
                if (entry.isDirectory || name !in allowed || entry.name.contains("..")) continue
                val out = File(tmp, name)
                out.outputStream().buffered().use { zin.copyTo(it) }
                if (out.length() <= 0L) throw IOException("语音引擎文件为空：$name")
                found += name
            }
        }
        if (found != allowed) {
            tmp.deleteRecursively()
            throw IOException("语音引擎包不完整：缺 ${(allowed - found).joinToString()}")
        }
        dest.deleteRecursively()
        dest.parentFile?.mkdirs()
        if (!tmp.renameTo(dest)) {
            tmp.copyRecursively(dest, overwrite = true)
            tmp.deleteRecursively()
        }
    }

    private fun publish(received: Long, total: Long) {
        val now = SystemClock.elapsedRealtime()
        if (now - lastProgressAt < 300) return
        lastProgressAt = now
        main.post { state = State.Running(received, total) }
    }
}
