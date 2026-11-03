// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语音引擎组件（2026-11-01 用户裁决）：只有完整包随安装包带原生库；普通包/基础包
//! 在「设置 → 语音」按需下载。组件清单（大小 + SHA-256）来自官网 `version.json`，
//! 下载整包校验通过后只解压四个库到用户模型目录的 `engine/` 下。

package xyz.erik.opi.candidate

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.FileOutputStream
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URI
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import java.nio.file.attribute.PosixFilePermissions
import java.security.MessageDigest
import java.util.zip.ZipInputStream

internal object SpeechEnginePack {
    sealed interface State {
        data object Idle : State
        data class Running(val received: Long, val total: Long) : State
        data object Done : State
        data class Failed(val message: String) : State
    }

    private val windows get() = isWindows()

    val libs: List<String>
        get() = if (windows) {
            listOf("opi_asr.dll", "sherpa-onnx-c-api.dll", "sherpa-onnx-cxx-api.dll", "onnxruntime.dll")
        } else {
            listOf("libopi_asr.so", "libsherpa-onnx-c-api.so", "libsherpa-onnx-cxx-api.so", "libonnxruntime.so")
        }

    fun root(): Path = speechModelRoot().resolve("engine")

    fun bundled(): Boolean = OPI_PACKAGE_TIER == "full"

    fun key(): String? = when {
        windows -> "asr-engine-windows-x64"
        isLinuxX64() -> "asr-engine-linux-x86_64"
        else -> null
    }

    fun fileName(): String? = key()?.let { "opi-$it.zip" }

    fun supported(): Boolean = key() != null

    fun ready(): Boolean = libs.all { Files.isRegularFile(root().resolve(it)) && Files.size(root().resolve(it)) > 0L }

    fun approxMb(): Int = 8

    var state by mutableStateOf<State>(State.Idle)
        private set

    @Volatile
    private var running = false

    fun download() {
        if (running || bundled() || ready()) return
        val key = key() ?: return
        val name = fileName() ?: return
        running = true
        state = State.Running(0, approxMb() * 1_048_576L)
        Thread({ runDownload(key, name) }, "opi-asr-engine-download").apply {
            isDaemon = true
            start()
        }
    }

    private data class ReleaseEngine(val url: String, val size: Long, val sha: String)

    /** GitHub release 上同名资产的权威大小/SHA；站点组件坏了时用它兜底。 */
    private fun releaseEngine(name: String): ReleaseEngine? {
        val text = httpGet(
            "https://api.github.com/repos/erikwang2013/opi/releases/tags/v$OPI_VERSION",
            15_000,
        ) ?: return null
        val root = parseLine(text) ?: return null
        val assets = (root["assets"] as? JVal.JArr)?.v ?: return null
        for (value in assets) {
            val asset = (value as? JVal.JObj)?.v ?: continue
            if ((asset["name"] as? JVal.JStr)?.v != name) continue
            val url = (asset["browser_download_url"] as? JVal.JStr)?.v
                ?.takeIf { it.startsWith("https://") } ?: continue
            val size = (asset["size"] as? JVal.JNum)?.v ?: continue
            val digest = (asset["digest"] as? JVal.JStr)?.v ?: continue
            val sha = digest.removePrefix("sha256:").takeIf {
                digest.startsWith("sha256:") && it.length == 64
            } ?: continue
            return ReleaseEngine(url, size, sha)
        }
        return null
    }

    private fun runDownload(key: String, name: String) {
        try {
            val text = httpGet(UPDATE_VERSION_URL)
                ?: throw IOException("无法读取组件清单（网络不可用）")
            val pkg = parseComponent(text, key)
                ?: throw IOException("组件清单缺少 $key，请先更新站点清单")
            val sha = pkg.sha256 ?: throw IOException("组件清单缺少 SHA-256")
            val size = pkg.size ?: throw IOException("组件清单缺少文件大小")
            state = State.Running(0, size)
            val tmpZip = Files.createTempFile("opi-asr-engine-", ".zip")
            try {
                try {
                    downloadVerified(pkg.url, tmpZip, size, sha)
                    install(tmpZip)
                } catch (site: Throwable) {
                    // 站点组件可能被截断/写坏：清单里的 SHA 与坏文件一致时，下载校验
                    // 会通过、解压才失败。此时改从 GitHub release 取同一版本的资产，
                    // 大小与 SHA 以 release API 的 asset digest 为准。
                    val release = releaseEngine(name) ?: throw site
                    downloadVerified(release.url, tmpZip, release.size, release.sha)
                    install(tmpZip)
                }
            } finally {
                Files.deleteIfExists(tmpZip)
            }
            state = State.Done
        } catch (t: Throwable) {
            state = State.Failed(t.message ?: t.javaClass.simpleName)
        } finally {
            running = false
        }
    }

    private fun downloadVerified(url: String, dest: Path, expected: Long, sha: String) {
        val conn = (URI(url).toURL().openConnection() as HttpURLConnection).apply {
            connectTimeout = 30_000
            readTimeout = 60_000
            instanceFollowRedirects = true
            setRequestProperty("User-Agent", "opi/$OPI_VERSION")
        }
        try {
            if (conn.responseCode !in 200..299) throw IOException("HTTP ${conn.responseCode}")
            val digest = MessageDigest.getInstance("SHA-256")
            var received = 0L
            conn.inputStream.use { input ->
                FileOutputStream(dest.toFile()).buffered().use { output ->
                    val buf = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        digest.update(buf, 0, n)
                        output.write(buf, 0, n)
                        received += n
                        if (received > expected) throw IOException("文件超长")
                        state = State.Running(received, expected)
                    }
                }
            }
            if (received != expected) throw IOException("文件大小不符：$received / $expected")
            val got = digest.digest().joinToString("") { "%02x".format(it.toInt() and 0xff) }
            if (!got.equals(sha, ignoreCase = true)) throw IOException("SHA-256 校验失败")
        } finally {
            conn.disconnect()
        }
    }

    private fun install(zip: Path) {
        val dest = root()
        val tmp = dest.resolveSibling(dest.fileName.toString() + ".tmp")
        tmp.toFile().deleteRecursively()
        Files.createDirectories(tmp)
        val allowed = libs.toSet()
        val found = mutableSetOf<String>()
        ZipInputStream(Files.newInputStream(zip)).use { zin ->
            while (true) {
                val entry = zin.nextEntry ?: break
                val name = entry.name.substringAfterLast('/')
                if (entry.isDirectory || name !in allowed || entry.name.contains("..")) continue
                val out = tmp.resolve(name)
                Files.newOutputStream(out).buffered().use { zin.copyTo(it) }
                if (Files.size(out) <= 0L) throw IOException("语音引擎文件为空：$name")
                runCatching {
                    Files.setPosixFilePermissions(out, PosixFilePermissions.fromString("rwx------"))
                }
                found += name
            }
        }
        if (found != allowed) {
            tmp.toFile().deleteRecursively()
            throw IOException("语音引擎包不完整：缺 ${(allowed - found).joinToString()}")
        }
        dest.toFile().deleteRecursively()
        Files.createDirectories(dest.parent)
        runCatching { Files.move(tmp, dest, StandardCopyOption.REPLACE_EXISTING) }
            .onFailure {
                tmp.toFile().copyRecursively(dest.toFile(), overwrite = true)
                tmp.toFile().deleteRecursively()
            }
        runCatching { Files.setPosixFilePermissions(dest, PosixFilePermissions.fromString("rwx------")) }
    }
}

private fun isLinuxX64(): Boolean =
    !isWindows() && !System.getProperty("os.name").orEmpty().lowercase().contains("mac") &&
        System.getProperty("os.arch").orEmpty().let { it.contains("amd64") || it.contains("x86_64") }
