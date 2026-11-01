// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

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
import xyz.erik.opi.engine.DictStore
import xyz.erik.opi.jni.EngineLoader
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean
import java.util.zip.ZipInputStream

/**
 * 基础包的可选词库包（2026-11-01 三档打包）：繁体、笔画、五笔 86/98、生僻字索引与
 * 康熙词库。普通包/完整包随安装包提供；基础包在设置页下载后解压到 `filesDir`，
 * EngineLoader 与生僻字面板优先读这里的文件。
 */
internal object DictPack {
    private const val TAG = "OpiDictPack"
    const val VERSION = "1.31.0"
    const val URL = "https://opi.erik.xyz/download/components/dicts/opi-dicts-$VERSION.zip"
    const val FALLBACK = "https://github.com/erikwang2013/opi/releases/download/v$VERSION/opi-dicts-$VERSION.zip"
    const val BYTES = 7_564_289L
    const val SHA256 = "719011206ad590d9b609b917ca465f3851200ed5a62a6e4b8f0c2e4eeb8b5d9f"

    /** 解压后落点（全部在 filesDir 根，与 EngineLoader 的 FILE_NAME 一致）。 */
    val FILES = listOf(
        "trad.opid", "stroke.opid", "wubi86.opid", "wubi98.opid",
        "rare_index.tsv", "kangxi.tsv",
    )

    sealed interface State {
        data object Idle : State
        data class Running(val received: Long, val total: Long) : State
        data object Done : State
        data class Failed(val message: String) : State
    }

    private val main = Handler(Looper.getMainLooper())
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val running = AtomicBoolean(false)
    @Volatile private var lastProgressAt = 0L

    var state by mutableStateOf<State>(State.Idle)
        private set

    fun installed(context: Context): Boolean =
        FILES.all { File(context.filesDir, it).length() > 0L }

    fun download(context: Context) {
        if (!running.compareAndSet(false, true)) return
        val app = context.applicationContext
        if (installed(app)) {
            state = State.Done
            running.set(false)
            return
        }
        state = State.Running(0, BYTES)
        scope.launch {
            try {
                install(app)
                runCatching {
                    val store = DictStore.from(app)
                    store.seedBundled("kangxi") {
                        File(app.filesDir, "kangxi.tsv").inputStream()
                    }
                    store.loadAll()
                }
                EngineLoader.io.execute { EngineLoader.load(app) }
                main.post {
                    state = State.Done
                    running.set(false)
                }
            } catch (t: Throwable) {
                Log.w(TAG, "dict pack download failed", t)
                main.post {
                    state = State.Failed(t.message ?: "下载失败")
                    running.set(false)
                }
            }
        }
    }

    private fun install(context: Context) {
        val zip = File(context.cacheDir, "opi-dicts-$VERSION.zip")
        try {
            downloadFile(URL, zip)
        } catch (t: Throwable) {
            downloadFile(FALLBACK, zip)
        }
        val digest = sha256(zip)
        if (digest != SHA256) {
            zip.delete()
            throw IOException("词库包校验失败（SHA-256 不符）")
        }
        extract(context, zip)
        zip.delete()
    }

    private fun downloadFile(url: String, dest: File) {
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
            var received = 0L
            conn.inputStream.use { input ->
                FileOutputStream(part).use { out ->
                    val buf = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        out.write(buf, 0, n)
                        received += n
                        publish(received)
                    }
                }
            }
            if (received != BYTES) throw IOException("词库包大小不符：$received，应 $BYTES")
            if (!part.renameTo(dest)) {
                part.copyTo(dest, overwrite = true)
                part.delete()
            }
        } finally {
            conn.disconnect()
            if (part.exists()) part.delete()
        }
    }

    private fun extract(context: Context, zip: File) {
        val allowed = FILES.toSet()
        val names = mutableSetOf<String>()
        ZipInputStream(zip.inputStream().buffered()).use { zin ->
            while (true) {
                val entry = zin.nextEntry ?: break
                val name = entry.name.substringAfterLast('/')
                if (entry.isDirectory || name !in allowed || entry.name.contains("..")) {
                    zin.closeEntry()
                    continue
                }
                val dest = File(context.filesDir, name)
                val part = File(context.filesDir, "$name.part")
                part.delete()
                FileOutputStream(part).use { out -> zin.copyTo(out) }
                if (!part.renameTo(dest)) {
                    part.copyTo(dest, overwrite = true)
                    part.delete()
                }
                names += name
                zin.closeEntry()
            }
        }
        val missing = allowed - names
        if (missing.isNotEmpty()) throw IOException("词库包缺文件：${missing.joinToString()}")
    }

    private fun publish(received: Long) {
        val now = SystemClock.elapsedRealtime()
        if (now - lastProgressAt < 300) return
        lastProgressAt = now
        main.post { state = State.Running(received, BYTES) }
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buf = ByteArray(256 * 1024)
            while (true) {
                val n = input.read(buf)
                if (n < 0) break
                digest.update(buf, 0, n)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it.toInt() and 0xff) }
    }

    fun mb(bytes: Long): String = String.format(java.util.Locale.ROOT, "%.1f MB", bytes / 1048576.0)
}
