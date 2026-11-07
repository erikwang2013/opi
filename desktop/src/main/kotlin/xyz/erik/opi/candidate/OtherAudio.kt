// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import com.sun.jna.platform.win32.User32
import com.sun.jna.platform.win32.WinDef
import com.sun.jna.platform.win32.WinUser
import java.util.concurrent.TimeUnit

/**
 * 录音时暂停其他音频/视频；停止录音后恢复**之前正在播的**那些。
 *
 * · Linux = MPRIS（`dbus-send`）：列出 `org.mpris.MediaPlayer2.*`，只暂停
 *   PlaybackStatus=Playing 的，记住名字；恢复时只 Play 这些 —— `playerctl -a play`
 *   会把用户本来暂停的播放器也点开，不能这么干。没有 `dbus-send` 时才退回 playerctl。
 * · Windows = `SendInput` 发一次 VK_MEDIA_PLAY_PAUSE（系统媒体会话的播放/暂停键）。
 *   旧实现的 PowerShell `SendKeys([char]173)` 会闪控制台且不一定送到系统媒体层。
 *
 * 都是尽力而为：工具/媒体会话不在时静默跳过，绝不影响录音本身。
 */
internal object OtherAudio {
    private var paused = false
    private val pausedPlayers = mutableListOf<String>()

    fun pause() {
        if (paused) return
        paused = true
        runCatching {
            if (isWindows()) {
                sendMediaPlayPause()
            } else {
                pauseLinux()
            }
        }
    }

    fun resume() {
        if (!paused) return
        paused = false
        runCatching {
            if (isWindows()) {
                sendMediaPlayPause()
            } else {
                resumeLinux()
            }
        }
    }

    // ---------- Linux：MPRIS ----------

    private fun pauseLinux() {
        val names = mprisPlayers()
        if (names.isEmpty()) {
            // 没有 MPRIS（或没有 dbus-send）：退回 playerctl（装了才有用）。
            runCommand("playerctl", "pause")
            return
        }
        // 总预算 3s：某个播放器卡住/不在线时别把「开始录音」拖成十几秒。
        val deadline = System.currentTimeMillis() + 3000
        for (name in names) {
            if (System.currentTimeMillis() > deadline) break
            if (!playbackIsPlaying(dbusProperty(name, "PlaybackStatus"))) continue
            if (runCommand(*mprisMethod(name, "Pause")) != null) pausedPlayers += name
        }
    }

    private fun resumeLinux() {
        for (name in pausedPlayers) {
            runCommand(*mprisMethod(name, "Play"))
        }
        pausedPlayers.clear()
    }

    private fun mprisPlayers(): List<String> {
        val out = runCommand(
            "dbus-send", "--session", "--print-reply",
            "--dest=org.freedesktop.DBus", "/org/freedesktop/DBus",
            "org.freedesktop.DBus.ListNames",
        ) ?: return emptyList()
        return mprisNames(out)
    }

    private fun dbusProperty(player: String, property: String): String? = runCommand(
        "dbus-send", "--session", "--print-reply", "--dest=$player",
        "/org/mpris/MediaPlayer2", "org.freedesktop.DBus.Properties.Get",
        "string:org.mpris.MediaPlayer2.Player", "string:$property",
    )

    private fun mprisMethod(player: String, method: String): Array<String> = arrayOf(
        "dbus-send", "--session", "--dest=$player",
        "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player.$method",
    )

    private fun runCommand(vararg argv: String): String? = try {
        val p = ProcessBuilder(*argv).redirectErrorStream(true).start()
        if (!p.waitFor(1500, TimeUnit.MILLISECONDS)) {
            p.destroyForcibly()
            null
        } else {
            val out = p.inputStream.bufferedReader().readText()
            if (p.exitValue() == 0) out else null
        }
    } catch (_: Exception) {
        null // 没装这个工具（IOException）/ 被系统拒绝：尽力而为
    }

    // ---------- Windows：系统媒体键 ----------

    private fun sendMediaPlayPause(): String? = try {
        val down = WinUser.INPUT()
        down.type = WinDef.DWORD(WinUser.INPUT.INPUT_KEYBOARD.toLong())
        down.input.setType("ki")
        down.input.ki.wVk = WinDef.WORD(VK_MEDIA_PLAY_PAUSE.toLong())
        down.input.ki.dwFlags = WinDef.DWORD(0L)
        val up = WinUser.INPUT()
        up.type = WinDef.DWORD(WinUser.INPUT.INPUT_KEYBOARD.toLong())
        up.input.setType("ki")
        up.input.ki.wVk = WinDef.WORD(VK_MEDIA_PLAY_PAUSE.toLong())
        up.input.ki.dwFlags = WinDef.DWORD(WinUser.KEYBDINPUT.KEYEVENTF_KEYUP.toLong())
        val n = User32.INSTANCE.SendInput(WinDef.DWORD(2L), arrayOf(down, up), down.size()).toInt()
        if (n == 2) null else "媒体键注入被拒绝（$n/2）"
    } catch (t: Throwable) {
        "媒体键注入失败：${t.javaClass.simpleName}"
    }
}

/** `dbus-send … ListNames` 的输出现象：`string "org.mpris.MediaPlayer2.xxx"` 若干行。 */
internal fun mprisNames(listNamesOutput: String): List<String> =
    Regex("string \"(org\\.mpris\\.MediaPlayer2\\.[^\"]+)\"")
        .findAll(listNamesOutput)
        .map { it.groupValues[1] }
        .distinct()
        .toList()

/** `Properties.Get … PlaybackStatus` 的回复里有没有 `"Playing"`。 */
internal fun playbackIsPlaying(propertyReply: String?): Boolean =
    propertyReply?.contains("\"Playing\"") == true

private const val VK_MEDIA_PLAY_PAUSE = 0xB3
