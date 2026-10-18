// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * FileProvider 配置与「自有独立」的**文本门禁**。
 *
 * 图片插入（commitContent/剪贴板回退）需要真机才能跑；JVM 侧能钉的是配置面：
 * provider/authority/路径与代码里的 authority 字面量、以及自有子页不回到文本通道
 * /最近使用。配置漂掉时这里先红，而不是等到真机上「点形态没反应」。
 */
class PetStickerConfigTest {

    private fun src(path: String): String = File(path).readText()

    @Test
    fun manifestDeclaresFileProviderForPetCache() {
        val manifest = src("src/main/AndroidManifest.xml")
        for (needle in listOf(
            """android:name="androidx.core.content.FileProvider"""",
            """android:authorities="${'$'}{applicationId}.fileprovider"""",
            """android:exported="false"""",
            """android:grantUriPermissions="true"""",
            """android:resource="@xml/file_paths"""",
        )) {
            assertTrue("manifest 缺：$needle", manifest.contains(needle))
        }
        // 代码里的 authority 与 manifest 的 ${applicationId}.fileprovider 同形。
        val commit = src("src/main/kotlin/xyz/erik/opi/PetStickerCommit.kt")
        assertTrue(
            "commitPetSticker 的 authority 与 manifest 不一致",
            commit.contains("\"\$packageName.fileprovider\""),
        )
    }

    @Test
    fun filePathsExposesOnlyThePetCacheDir() {
        val paths = src("src/main/res/xml/file_paths.xml")
        assertTrue(
            "file_paths 必须声明 cache-path name=pet path=pet/",
            Regex("""<cache-path\b[^>]*\bname="pet"[^>]*\bpath="pet/"""").containsMatchIn(paths),
        )
    }

    @Test
    fun emojiSubPageDoesNotOwnPetGrid() {
        val panel = src("src/main/kotlin/xyz/erik/opi/keyboard/EmojiPanel.kt")
        assertFalse(
            "Emoji 子页不得再引用 PetEmoji —— 自有是独立顶级子页",
            panel.contains("PetEmoji"),
        )
        assertTrue("自有子页必须接图片回调", panel.contains("PetStickerPanel(onPickPet)"))
    }

    @Test
    fun unsupportedEditorFallsBackToTextTagNotClipboardImage() {
        val commit = src("src/main/kotlin/xyz/erik/opi/PetStickerCommit.kt")
        assertTrue(
            "回退必须上屏独立文本标签 [小欧·标签]",
            commit.contains("\"[小欧·\${PetEmoji.label(mood)}]\""),
        )
        assertTrue("回退 toast 文案缺失", commit.contains("当前应用不支持插入图片，已用文字标签"))
        assertFalse("不得回退到剪贴板图片", commit.contains("ClipboardManager"))
        assertFalse("不得回退到 clipboard URI", commit.contains("newUri"))
        assertFalse("不得回退到笑脸 emoji", commit.contains("😊") || commit.contains("😄"))
    }

    @Test
    fun petCellsNeverEnterTextChannelOrRecents() {
        val panel = src("src/main/kotlin/xyz/erik/opi/keyboard/PetStickerPanel.kt")
        assertFalse("宠物点击不得写最近使用（recents 只收 Emoji 文本）", panel.contains("recordRecent"))
        assertFalse("宠物点击必须走 onPick，不得走文本提交", panel.contains("onCommit"))
        val service = src("src/main/kotlin/xyz/erik/opi/OpiImeService.kt")
        assertTrue("服务必须把 onPickPet 接到图片通道", service.contains("commitPetSticker"))
    }
}
