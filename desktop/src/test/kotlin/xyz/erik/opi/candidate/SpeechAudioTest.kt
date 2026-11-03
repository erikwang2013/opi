// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

class SpeechAudioTest {
    private fun le16(v: Int): ByteArray =
        byteArrayOf((v and 0xff).toByte(), ((v shr 8) and 0xff).toByte())

    @Test
    fun sixteen_kilo_mono_passes_through() {
        val src = le16(1234) + le16(-2345)
        assertArrayEquals(src, pcm16To16kMono(src, src.size, 16_000, 1))
    }

    @Test
    fun stereo_is_averaged_and_downsampled() {
        // 48 kHz 立体声两帧 → 一帧 16 kHz；左 1000、右 3000 → 2000。
        val src = le16(1000) + le16(3000) + le16(1000) + le16(3000)
        val out = pcm16To16kMono(src, src.size, 48_000, 2)
        assertEquals(2, out.size)
        assertEquals(2000, (out[0].toInt() and 0xff) or (out[1].toInt() shl 8))
    }

    @Test
    fun empty_or_partial_frame_is_safe() {
        assertEquals(0, pcm16To16kMono(ByteArray(0), 0, 48_000, 2).size)
        assertEquals(0, pcm16To16kMono(ByteArray(3), 3, 48_000, 2).size)
    }
}
