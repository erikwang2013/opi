// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class OtherAudioTest {
    @Test
    fun `mpris player names come from ListNames reply`() {
        val reply = """
            method return time=1 sender=org.freedesktop.DBus -> destination=:1.2 serial=3 reply_serial=2
               array [
                  string "org.freedesktop.DBus"
                  string "org.mpris.MediaPlayer2.firefox.instance_1_2"
                  string "org.mpris.MediaPlayer2.vlc"
                  string "org.deepin.dde.daemon.Clipboard1"
                  string "org.mpris.MediaPlayer2.vlc"
               ]
        """.trimIndent()
        assertEquals(
            listOf("org.mpris.MediaPlayer2.firefox.instance_1_2", "org.mpris.MediaPlayer2.vlc"),
            mprisNames(reply),
        )
        assertEquals(emptyList<String>(), mprisNames(""))
    }

    @Test
    fun `playback status only counts playing`() {
        assertTrue(playbackIsPlaying("""   variant       string "Playing""""))
        assertFalse(playbackIsPlaying("""   variant       string "Paused""""))
        assertFalse(playbackIsPlaying("   variant       string \"Stopped\""))
        assertFalse(playbackIsPlaying(null))
    }
}
