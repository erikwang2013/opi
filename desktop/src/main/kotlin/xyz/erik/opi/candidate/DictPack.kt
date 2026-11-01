// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URI
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean
import java.util.zip.ZipInputStream
import kotlin.concurrent.thread

/**
 * 基础包的可选词库包（2026-11-01 三档打包）：繁体、笔画、五笔 86/98、生僻字索引、
 * 康熙词库。解压到 `%LOCALAPPDATA%\opi`（Windows TSF）或 `$XDG_DATA_HOME/opi`
 * （Linux fcitx5），两张引擎都按既有 Type::Data / LocalAppData 路径查找。
 */
internal object DictPack {
    const val VERSION = "1.31.0"
    const val URL = "https://opi.erik.xyz/download/components/dicts/opi-dicts-$VERSION.zip"
    const val FALLBACK =
        "https://github.com/erikwang2013/opi/releases/download/v$VERSION/opi-dicts-$VERSION.zip"
    const val BYTES = 7_564_289L
    const val SHA256 = "719011206ad590d9b609b917ca465f3851200ed5a62a6e4b8f0c2e4eeb8b5d9f"

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

    var state by mutableStateOf<State>(State.Idle)
        private set

    private val running = AtomicBoolean(false)

    fun installRoot(): Path = speechModelRoot().parent

    /** 用户目录里已装（基础包下载后）。 */
    fun installed(): Boolean = FILES.all { Files.isRegularFile(installRoot().resolve(it)) }

    /** 普通/完整包随安装包或系统包提供（Windows app 目录 / Linux /usr/share/opi）。 */
    fun packaged(): Boolean {
        val candidates = mutableListOf<Path>()
        System.getProperty("jpackage.app-path")?.let {
            runCatching { Paths.get(it).parent }.getOrNull()?.let { p -> candidates.add(p) }
        }
        val xdg = System.getenv("XDG_DATA_HOME")?.takeIf { it.isNotEmpty() }
            ?: Paths.get(System.getProperty("user.home") ?: ".", ".local", "share").toString()
        candidates.add(Paths.get(xdg, "opi"))
        candidates.add(Paths.get("/usr/share/opi"))
        candidates.add(Paths.get("/usr/local/share/opi"))
        return candidates.any { Files.isRegularFile(it.resolve("trad.opid")) }
    }

    fun download() {
        if (!running.compareAndSet(false, true)) return
        if (installed()) {
            state = State.Done
            running.set(false)
            return
        }
        state = State.Running(0, BYTES)
        thread(name = "opi-dict-pack", isDaemon = true) {
            try {
                install()
                state = State.Done
            } catch (t: Throwable) {
                state = State.Failed(t.message ?: "下载失败")
            } finally {
                running.set(false)
            }
        }
    }

    private fun install() {
        val dir = installRoot()
        Files.createDirectories(dir)
        val zip = Files.createTempFile("opi-dicts-", ".zip").toFile()
        try {
            try {
                downloadFile(URL, zip)
            } catch (_: Throwable) {
                downloadFile(FALLBACK, zip)
            }
            if (sha256(zip) != SHA256) error("词库包校验失败（SHA-256 不符）")
            extract(zip, dir)
        } finally {
            zip.delete()
        }
    }

    private fun downloadFile(url: String, dest: File) {
        val conn = (URI(url).toURL().openConnection() as HttpURLConnection).apply {
            connectTimeout = 30_000
            readTimeout = 60_000
            instanceFollowRedirects = true
            setRequestProperty("User-Agent", "OPI-Desktop")
        }
        try {
            val code = conn.responseCode
            if (code !in 200..299) error("HTTP $code")
            var received = 0L
            conn.inputStream.use { input ->
                FileOutputStream(dest).use { out ->
                    val buf = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        out.write(buf, 0, n)
                        received += n
                        if (received % (512 * 1024) < 64 * 1024) {
                            state = State.Running(received, BYTES)
                        }
                    }
                }
            }
            if (received != BYTES) error("词库包大小不符：$received，应 $BYTES")
        } finally {
            conn.disconnect()
        }
    }

    private fun extract(zip: File, dir: Path) {
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
                Files.copy(zin, dir.resolve(name), java.nio.file.StandardCopyOption.REPLACE_EXISTING)
                names += name
                zin.closeEntry()
            }
        }
        val missing = allowed - names
        if (missing.isNotEmpty()) error("词库包缺文件：${missing.joinToString()}")
    }

    private fun sha256(file: File): String {
        val md = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buf = ByteArray(256 * 1024)
            while (true) {
                val n = input.read(buf)
                if (n < 0) break
                md.update(buf, 0, n)
            }
        }
        return md.digest().joinToString("") { "%02x".format(it.toInt() and 0xff) }
    }

    fun mb(bytes: Long): Int = ((bytes + 1048575) / 1048576).toInt()
}
