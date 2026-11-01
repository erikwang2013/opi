// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

/** 录音时暂停其他音频：Linux 走 playerctl，Windows 发一次媒体播放/暂停键；恢复时反向。 */
internal object OtherAudio {
    private var paused = false

    fun pause() {
        if (paused) return
        paused = true
        runCatching {
            if (System.getProperty("os.name").lowercase().contains("win")) {
                ProcessBuilder(
                    "powershell", "-NoProfile", "-Command",
                    "(New-Object -ComObject WScript.Shell).SendKeys([char]173)",
                ).start()
            } else {
                ProcessBuilder("playerctl", "pause").start()
            }
        }
    }

    fun resume() {
        if (!paused) return
        paused = false
        runCatching {
            if (System.getProperty("os.name").lowercase().contains("win")) {
                ProcessBuilder(
                    "powershell", "-NoProfile", "-Command",
                    "(New-Object -ComObject WScript.Shell).SendKeys([char]173)",
                ).start()
            } else {
                ProcessBuilder("playerctl", "play").start()
            }
        }
    }
}

