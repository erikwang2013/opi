// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Path
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 宿主文件路径与读写的纯 JVM 测试（无 Compose/桌面窗）：路径必须与 Rust 两轨一致，
 * 缺失/坏文件必须回默认值，保存必须原子且能读回（含 enabled=false 这种「关掉开关」
 * 的宿主态 —— 丢了它，重启后短语会自己重新启用）。
 */
class CustomPhraseStoreTest {

    private fun tempDir(): Path = Files.createTempDirectory("opi-phrases-test")

    @Test
    fun linuxPathFollowsXdgDataHomeAndHomeFallback() {
        assertEquals(
            Path.of("/xdg/opi", CUSTOM_PHRASES_FILE_NAME),
            customPhrasesPathFor(false, "/xdg", "/home/u", null),
        )
        // 空串等同未设置（XDG 规范；与 Rust xdg_data_dir、dictsDirFor 同口径）
        assertEquals(
            Path.of("/home/u/.local/share/opi", CUSTOM_PHRASES_FILE_NAME),
            customPhrasesPathFor(false, "", "/home/u", null),
        )
        assertEquals(
            Path.of("/home/u/.local/share/opi", CUSTOM_PHRASES_FILE_NAME),
            customPhrasesPathFor(false, null, "/home/u", "/ignored"),
        )
    }

    @Test
    fun windowsPathIsLocalAppDataOpi() {
        assertEquals(
            Path.of("/lad/opi", CUSTOM_PHRASES_FILE_NAME),
            customPhrasesPathFor(true, "/xdg", "/home/u", "/lad"),
        )
    }

    @Test
    fun englishPathIsTheSiblingFileOnBothPlatforms() {
        assertEquals(
            Path.of("/xdg/opi", CUSTOM_PHRASES_EN_FILE_NAME),
            englishCustomPhrasesPathFor(false, "/xdg", "/home/u", null),
        )
        assertEquals(
            Path.of("/home/u/.local/share/opi", CUSTOM_PHRASES_EN_FILE_NAME),
            englishCustomPhrasesPathFor(false, "", "/home/u", null),
        )
        assertEquals(
            Path.of("/lad/opi", CUSTOM_PHRASES_EN_FILE_NAME),
            englishCustomPhrasesPathFor(true, "/xdg", "/home/u", "/lad"),
        )
        // 中文路径不受影响（两套文件必须分开）
        assertEquals(
            Path.of("/xdg/opi", CUSTOM_PHRASES_FILE_NAME),
            customPhrasesPathFor(false, "/xdg", "/home/u", null),
        )
    }

    @Test
    fun missingFileLoadsEnabledEmptyDefault() {
        val path = tempDir().resolve("absent.json")
        assertEquals(CustomPhraseState(), loadCustomPhrases(path))
    }

    @Test
    fun brokenFileLoadsDefaultWithoutThrowing() {
        val path = tempDir().resolve(CUSTOM_PHRASES_FILE_NAME)
        Files.writeString(path, """{"version":1,"enabled":false,"phrases":[{"trigger":"1","text":"x"}]}""")
        assertEquals(CustomPhraseState(), loadCustomPhrases(path))
    }

    @Test
    fun saveCreatesParentAndRoundTripsHostState() {
        val path = tempDir().resolve("nested/dir/$CUSTOM_PHRASES_FILE_NAME")
        val state = CustomPhraseState(
            enabled = false,
            phrases = listOf(CustomPhrase("dz", "地址"), CustomPhrase("hi", "say \"hi\"\nline2")),
        )
        assertNull(saveCustomPhrases(state, path))
        assertEquals(state, loadCustomPhrases(path))
        // 唯一临时名 + rename：保存完目录里只剩目标文件（没有 .tmp 残骸）
        val left = Files.newDirectoryStream(path.parent).use { s ->
            s.map { it.fileName.toString() }
        }
        assertEquals(listOf(CUSTOM_PHRASES_FILE_NAME), left)
    }

    @Test
    fun saveOverwritesAtomicallyAndKeepsOnlyOneFile() {
        val dir = tempDir()
        val path = dir.resolve(CUSTOM_PHRASES_FILE_NAME)
        assertNull(saveCustomPhrases(CustomPhraseState(phrases = listOf(CustomPhrase("aa", "一"))), path))
        assertNull(saveCustomPhrases(CustomPhraseState(phrases = listOf(CustomPhrase("bb", "二"))), path))
        assertEquals(
            listOf(CustomPhrase("bb", "二")),
            loadCustomPhrases(path).phrases,
        )
        assertEquals(
            1,
            Files.newDirectoryStream(dir).use { s -> s.count() },
        )
    }

    @Test
    fun englishAndChineseFilesAreIndependent() {
        val dir = tempDir()
        val cn = dir.resolve(CUSTOM_PHRASES_FILE_NAME)
        val en = dir.resolve(CUSTOM_PHRASES_EN_FILE_NAME)
        val cnState = CustomPhraseState(phrases = listOf(CustomPhrase("dz", "地址")))
        val enState = CustomPhraseState(
            enabled = false,
            phrases = listOf(CustomPhrase("hello", "hello there")),
        )
        assertNull(saveCustomPhrases(cnState, cn))
        assertNull(saveEnglishCustomPhrases(enState, en))
        assertEquals(cnState, loadCustomPhrases(cn))
        assertEquals(enState, loadEnglishCustomPhrases(en))

        // 只覆盖英文文件：中文原样、两个文件都还在。
        val en2 = CustomPhraseState(phrases = listOf(CustomPhrase("bye", "goodbye")))
        assertNull(saveEnglishCustomPhrases(en2, en))
        assertEquals(cnState, loadCustomPhrases(cn))
        assertEquals(en2, loadEnglishCustomPhrases(en))
        assertEquals(
            setOf(CUSTOM_PHRASES_FILE_NAME, CUSTOM_PHRASES_EN_FILE_NAME),
            Files.newDirectoryStream(dir).use { s -> s.map { it.fileName.toString() }.toSet() },
        )
    }

    @Test
    fun brokenEnglishFileDefaultsAndLeavesChineseAlone() {
        val dir = tempDir()
        val cn = dir.resolve(CUSTOM_PHRASES_FILE_NAME)
        val en = dir.resolve(CUSTOM_PHRASES_EN_FILE_NAME)
        val cnState = CustomPhraseState(phrases = listOf(CustomPhrase("dz", "地址")))
        assertNull(saveCustomPhrases(cnState, cn))
        Files.writeString(en, "{")
        assertEquals(CustomPhraseState(), loadEnglishCustomPhrases(en))
        assertEquals(cnState, loadCustomPhrases(cn))
    }
}
