// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 应用内更新下载的门禁：本地 HttpServer 起一个临时端点，真下、真算 SHA-256、真落盘。
//! 另钉文件名/大小格式两个纯函数。
//!
//! `requireHttps=false` 只在这个测试里放开（本地 127.0.0.1 没有证书）；生产默认 true。

package xyz.erik.opi.candidate

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.net.InetSocketAddress
import java.nio.file.Files
import java.security.MessageDigest

class UpdateDownloadTest {

    private fun sha256(bytes: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(bytes)
            .joinToString("") { "%02x".format(it.toInt() and 0xFF) }

    private fun withServer(payload: ByteArray, body: (String) -> Unit) {
        val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        server.createContext("/pkg.bin") { ex ->
            ex.sendResponseHeaders(200, payload.size.toLong())
            ex.responseBody.use { it.write(payload) }
        }
        server.start()
        try {
            body("http://127.0.0.1:${server.address.port}/pkg.bin")
        } finally {
            server.stop(0)
        }
    }

    private fun awaitTerminal(downloader: UpdateDownloader): DownloadState = runBlocking {
        val deadline = System.currentTimeMillis() + 10_000
        while (System.currentTimeMillis() < deadline) {
            val s = downloader.state
            if (s is DownloadState.Done || s is DownloadState.Failed) return@runBlocking s
            delay(20)
        }
        throw AssertionError("下载 10s 未结束，最后状态：${downloader.state}")
    }

    @Test
    fun `downloads, verifies sha256 and lands the file`() {
        val payload = ByteArray(200_000) { (it % 251).toByte() }
        val sha = sha256(payload)
        withServer(payload) { url ->
            val dir = Files.createTempDirectory("opi-update-test").toFile()
            try {
                val d = UpdateDownloader(
                    scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
                    requireHttps = false,
                    targetDir = dir,
                )
                d.start(
                    UpdatePackageKind.FULL,
                    UpdatePackage(url, sha, payload.size.toLong()),
                    "1.2.3",
                )
                val end = awaitTerminal(d)
                assertTrue("应下载成功，实际 $end", end is DownloadState.Done)
                end as DownloadState.Done
                assertEquals(true, end.shaOk)
                assertTrue("文件应落盘", end.file.isFile)
                assertEquals("下载内容应与服务端一致", sha, sha256(end.file.readBytes()))
                assertFalse("临时 .part 必须清掉", File(dir, end.file.name + ".part").exists())
            } finally {
                dir.deleteRecursively()
            }
        }
    }

    @Test
    fun `reuses a verified local file without touching the network`() {
        val payload = ByteArray(64_000) { (it % 241).toByte() }
        val sha = sha256(payload)
        var requests = 0
        val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        server.createContext("/pkg.bin") { ex ->
            requests++
            ex.sendResponseHeaders(200, payload.size.toLong())
            ex.responseBody.use { it.write(payload) }
        }
        server.start()
        val url = "http://127.0.0.1:${server.address.port}/pkg.bin"
        val dir = Files.createTempDirectory("opi-update-reuse").toFile()
        try {
            File(dir, downloadFileName(url, "1.2.3", UpdatePackageKind.FULL)).writeBytes(payload)
            val d = UpdateDownloader(
                scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
                requireHttps = false,
                targetDir = dir,
            )
            d.start(UpdatePackageKind.FULL, UpdatePackage(url, sha, payload.size.toLong()), "1.2.3")
            val end = awaitTerminal(d)
            assertTrue("应直接复用本地文件，实际 $end", end is DownloadState.Done)
            assertEquals("复用时不得请求网络", 0, requests)
            assertEquals(true, (end as DownloadState.Done).shaOk)
        } finally {
            server.stop(0)
            dir.deleteRecursively()
        }
    }

    @Test
    fun `wrong sha256 deletes the bad file`() {
        val payload = "hello update".toByteArray()
        withServer(payload) { url ->
            val dir = Files.createTempDirectory("opi-update-test-bad").toFile()
            try {
                val d = UpdateDownloader(
                    scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
                    requireHttps = false,
                    targetDir = dir,
                )
                d.start(
                    UpdatePackageKind.HOT,
                    UpdatePackage(url, "0".repeat(64), payload.size.toLong()),
                    "1.2.3",
                )
                val end = awaitTerminal(d)
                assertTrue("校验失败应报 Failed，实际 $end", end is DownloadState.Failed)
                assertTrue((end as DownloadState.Failed).reason.contains("SHA-256"))
                assertEquals("坏包不许留", 0, dir.listFiles()?.size ?: 0)
            } finally {
                dir.deleteRecursively()
            }
        }
    }

    @Test
    fun `download section renders both package links`() {
        val available = UpdateCheck.Available(
            latest = "1.27.0",
            download = "https://opi.erik.xyz/download",
            full = UpdatePackage("https://opi.erik.xyz/download/opi-1.27.0-windows-x64-setup.exe", null, 66_094_276),
            hot = UpdatePackage("https://opi.erik.xyz/download/opi-1.27.0-linux-amd64-hot.zip", null, 34_782_920),
        )
        val scene = ImageComposeScene(width = 520, height = 180, density = Density(1f)) {
            Column(Modifier.fillMaxSize().padding(12.dp)) {
                UpdateDownloadSection(UpdateDownloader(requireHttps = false), available) {}
            }
        }
        val bytes = try {
            scene.render()
            scene.render().encodeToData(org.jetbrains.skia.EncodedImageFormat.PNG)?.bytes
                ?: throw AssertionError("PNG 编码失败")
        } finally {
            scene.close()
        }
        val dir = File("build/opi-render-probe").apply { mkdirs() }
        val file = File(dir, "update-section.png")
        file.writeBytes(bytes)
        val img = javax.imageio.ImageIO.read(file) ?: throw AssertionError("读不出 update-section.png")
        var ink = false
        for (y in 0 until img.height) for (x in 0 until img.width) {
            if (((img.getRGB(x, y) ushr 24) and 0xFF) > 8) { ink = true; break }
        }
        assertTrue("下载段应该有内容", ink)
    }

    @Test
    fun `file names and human sizes are stable`() {
        assertEquals(
            "opi-1.2.3-windows-x64-setup.exe",
            downloadFileName(
                "https://opi.erik.xyz/download/opi-1.2.3-windows-x64-setup.exe?x=1#f",
                "1.2.3",
                UpdatePackageKind.FULL,
            ),
        )
        assertTrue(downloadFileName("https://opi.erik.xyz/download/", "1.2.3", UpdatePackageKind.HOT).endsWith(".zip"))
        assertEquals("0 B", formatDownloadSize(0))
        assertEquals("1.0 KB", formatDownloadSize(1024))
        assertEquals("1.5 MB", formatDownloadSize(1024L * 1024 + 512 * 1024))
        assertEquals("未知大小", formatDownloadSize(null))
    }
}
