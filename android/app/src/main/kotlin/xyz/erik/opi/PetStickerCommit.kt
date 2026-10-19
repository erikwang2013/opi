// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 2026-10-18：自有表情改插入 PNG（用户裁决）。OpiImeService 已 494 行（500 行门禁），
// 图片通道整体拆在这里 —— 扩展函数，读的仍是 service 的成员（同 ComposingText.kt 先例）。

package xyz.erik.opi

import android.content.ClipDescription
import android.util.Log
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.widget.Toast
import androidx.core.content.FileProvider
import androidx.core.view.inputmethod.InputConnectionCompat
import androidx.core.view.inputmethod.InputContentInfoCompat
import xyz.erik.opi.ime.PendingCommit
import xyz.erik.opi.keyboard.PetEmoji
import xyz.erik.opi.pet.PetFontAssets
import xyz.erik.opi.pet.PetEmojiFont
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetStickerAssets
import xyz.erik.opi.settings.ImePreferences
import java.io.File
import java.io.IOException

private const val TAG = "OpiPetSticker"

/**
 * 「自有」宠物形态 = **插入图片**，不提交任何 Unicode 文本（2026-10-18 用户裁决）。
 * 例外：用户显式打开 PUA 开关时上屏字体字符（2026-10-19，见 [commitPetStickerTo]）。
 *
 * 图片走 `InputConnection.commitContent`，格式优先级（2026-10-19）：**GIF → PNG →
 * 文本标签**。图片通道被宿主拒绝时上屏 `[小欧·笑]` 这样的独立标签，**不再退回
 * Unicode emoji**（2026-10-19 用户裁决：emoji 与自有表情分不清）。GIF 是同一张
 * PNG 的轻微弹跳/缩放动画，见 `shared/pet/stickers-gif/`。
 * IC 为空时与文本提交同一套延迟重试（[PendingCommit.RETRY_MS]），仍拿不到就放弃并记日志。
 */
internal fun OpiImeService.commitPetSticker(mood: PetMood) {
    val ic = currentInputConnection
    if (ic != null) {
        commitPetStickerTo(ic, mood)
        return
    }
    Log.w(TAG, "IC null, retry pet sticker in ${PendingCommit.RETRY_MS}ms: ${mood.name}")
    pendingCommit.schedule {
        val ic2 = currentInputConnection
        if (ic2 != null) {
            commitPetStickerTo(ic2, mood)
        } else {
            Log.w(TAG, "IC still null, pet sticker dropped: ${mood.name}")
        }
    }
}

private fun OpiImeService.commitPetStickerTo(ic: InputConnection, mood: PetMood) {
    // PUA 开关（2026-10-19，缺省关）：上屏 map.tsv 的字体字符。表缺失时回退图片通道 ——
    // 一个坏资产不该让开关变成「点了没反应」。
    if (ImePreferences.from(this).puaEmoji) {
        val pua = PetFontAssets.preload(assets)?.let { PetEmojiFont.puaFor(it, mood) }
        if (pua != null) {
            dropComposing(ic)
            ic.commitText(pua, 1)
            return
        }
        Log.w(TAG, "PUA 已开但 map.tsv 不可用，回退图片通道: ${mood.name}")
    }
    // 优先动图（2026-10-19），编辑器不收 GIF 再退回 PNG，最后才是文本标签。
    val gif = petStickerFile(PetEmoji.gifAssetPath(mood), PetStickerAssets.gifFile(mood))
    val png = petStickerFile(PetEmoji.assetPath(mood), PetStickerAssets.stickerFile(mood))
    if (gif == null && png == null) {
        toast("小欧表情暂不可用")
        return
    }
    dropComposing(ic)
    if (gif != null && commitPetStickerImage(ic, gif, "image/gif")) return
    if (png != null && commitPetStickerImage(ic, png, "image/png")) return
    Log.w(TAG, "editor rejected image/gif and image/png, fallback to text tag: ${mood.name}")
    commitPetStickerTextFallback(ic, mood)
}

/** 把一张图通过 commitContent 交给编辑器；FileProvider/编辑器失败都只返回 false。 */
private fun OpiImeService.commitPetStickerImage(
    ic: InputConnection,
    file: File,
    mime: String,
): Boolean {
    val uri = try {
        FileProvider.getUriForFile(this, "$packageName.fileprovider", file)
    } catch (e: IllegalArgumentException) {
        // file_paths.xml 与 cacheDir/pet 对不上（配置错误）—— 不让 IME 崩，交给下一档回退。
        Log.w(TAG, "FileProvider 配置错误，跳过 $mime：$e")
        return false
    }
    val info = InputContentInfoCompat(
        uri,
        ClipDescription("小欧表情", arrayOf(mime)),
        null,
    )
    // currentInputEditorInfo 正常非空；旧 API 的兼容路径会读它，空 EditorInfo 只会让它返回 false。
    val ok = InputConnectionCompat.commitContent(
        ic,
        currentInputEditorInfo ?: EditorInfo(),
        info,
        InputConnection.INPUT_CONTENT_GRANT_READ_URI_PERMISSION,
        null,
    )
    if (ok) {
        Log.i(TAG, "pet sticker inserted: ${file.name} ($mime)")
    } else {
        Log.w(TAG, "editor rejected $mime: ${file.name}")
    }
    return ok
}

/**
 * assets 路径 → cacheDir/pet（存在且非空则跳过，供 FileProvider 暴露读权限）。
 * 0 字节 = 上次拷贝被中断留下的残骸；`exists()` 为真但图是坏的，必须重拷。
 * 缺资产返回 null（GIF 缺失是可回退的情况，由调用方决定是否 toast）。
 */
private fun OpiImeService.petStickerFile(assetPath: String, name: String): File? {
    val file = File(File(cacheDir, "pet").apply { mkdirs() }, name)
    if (file.length() > 0) return file
    return try {
        assets.open(assetPath).use { input -> file.outputStream().use { input.copyTo(it) } }
        file
    } catch (e: IOException) {
        Log.w(TAG, "pet sticker asset missing: $assetPath", e)
        null
    }
}

/**
 * 编辑器不支持插入图片时的回退：上屏**独立文本标签** `[小欧·笑]`，绝不退回笑脸 emoji
 * （2026-10-19 用户裁决：emoji 和自有表情在输入框里分不清）。
 *
 * 不走剪贴板图片：FileProvider 的读权限授不到任意粘贴方，会出现「提示已复制却粘不出来」。
 */
private fun OpiImeService.commitPetStickerTextFallback(ic: InputConnection, mood: PetMood) {
    dropComposing(ic)
    // 标签本身已带「小欧·」前缀（PetEmojiMap 是唯一源），这里只加方括号。
    ic.commitText("[${PetEmoji.label(mood)}]", 1)
    toast("当前应用不支持插入图片，已用文字标签")
}

private fun OpiImeService.toast(text: String) {
    Toast.makeText(this, text, Toast.LENGTH_SHORT).show()
}
