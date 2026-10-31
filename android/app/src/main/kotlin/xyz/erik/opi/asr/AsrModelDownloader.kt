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
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean

internal sealed interface AsrDownloadState {
    data object Idle : AsrDownloadState
    data class Running(val packId: String, val received: Long, val total: Long) : AsrDownloadState
    data class Done(val packId: String) : AsrDownloadState
    data class Failed(val packId: String, val message: String) : AsrDownloadState
}

/**
 * 模型下载器（设置页入口）。一个进程内单例：离开设置页/旋转屏幕不打断下载。
 *
 * 不用 DownloadManager：它只能写外部私有目录，而模型目录要求是 `filesDir`；
 * 直接用 HttpURLConnection 流式写入，边下边算 SHA-256，校验通过才改名。
 */
internal object AsrModelDownloader {
    private const val TAG = "OpiAsr"
    private val main = Handler(Looper.getMainLooper())
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val running = AtomicBoolean(false)

    @Volatile
    private var lastProgressAt = 0L

    var state by mutableStateOf<AsrDownloadState>(AsrDownloadState.Idle)
        private set

    fun download(context: Context, packId: String) {
        if (!running.compareAndSet(false, true)) return
        val app = context.applicationContext
        val pack = AsrModels.pack(packId)
        if (AsrModels.packReady(app, pack)) {
            state = AsrDownloadState.Done(packId)
            running.set(false)
            return
        }
        val total = pack.bytes + AsrModels.VAD_BYTES
        state = AsrDownloadState.Running(packId, 0, total)
        scope.launch {
            try {
                install(app, pack, total)
                main.post {
                    state = AsrDownloadState.Done(packId)
                    running.set(false)
                }
            } catch (t: Throwable) {
                Log.w(TAG, "model download failed", t)
                val message = t.message ?: "下载失败"
                main.post {
                    state = AsrDownloadState.Failed(packId, message)
                    running.set(false)
                }
            }
        }
    }

    private fun install(context: Context, pack: AsrModelPack, total: Long) {
        var done = 0L
        val vad = AsrModels.vadFile(context)
        if (AsrModels.vadReady(context)) {
            done += AsrModels.VAD_BYTES
        } else {
            downloadFile(
                AsrModels.vadUrl(), vad, AsrModels.VAD_BYTES, AsrModels.VAD_SHA256,
            ) { received -> publish(pack.id, done + received, total) }
            done += AsrModels.VAD_BYTES
        }

        val dir = AsrModels.packDir(context, pack).apply { mkdirs() }
        for (f in pack.files) {
            val dest = File(dir, f.path)
            // 已完成文件由本下载器原子写入且当时校验过；.part 不会命中这个尺寸。
            if (dest.length() == f.bytes) {
                done += f.bytes
                continue
            }
            downloadFile(f.url, dest, f.bytes, f.sha256) { received ->
                publish(pack.id, done + received, total)
            }
            done += f.bytes
        }
        writeManifest(dir, pack)
        main.post { state = AsrDownloadState.Running(pack.id, total, total) }
    }

    private fun publish(packId: String, received: Long, total: Long) {
        val now = SystemClock.elapsedRealtime()
        if (now - lastProgressAt < 300) return
        lastProgressAt = now
        main.post { state = AsrDownloadState.Running(packId, received, total) }
    }

    private fun downloadFile(
        url: String,
        dest: File,
        expectedBytes: Long,
        expectedSha256: String,
        onProgress: (Long) -> Unit,
    ) {
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
                        onProgress(received)
                    }
                }
            }
            if (received != expectedBytes) {
                throw IOException("文件大小不符：下载 $received，应 $expectedBytes")
            }
            val got = hex(digest.digest())
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

    private fun writeManifest(dir: File, pack: AsrModelPack) {
        val files = JSONArray()
        for (f in pack.files) {
            files.put(
                JSONObject()
                    .put("path", f.path)
                    .put("size", f.bytes)
                    .put("sha256", f.sha256)
                    .put("role", f.role),
            )
        }
        val json = JSONObject().put("version", 1).put("files", files).toString()
        val tmp = File(dir, "manifest.json.part")
        tmp.writeText(json)
        val manifest = File(dir, "manifest.json")
        if (!tmp.renameTo(manifest)) {
            manifest.delete()
            if (!tmp.renameTo(manifest)) {
                tmp.copyTo(manifest, overwrite = true)
                tmp.delete()
            }
        }
    }

    private fun hex(bytes: ByteArray): String = buildString(bytes.size * 2) {
        for (b in bytes) append("%02x".format(b.toInt() and 0xff))
    }
}
