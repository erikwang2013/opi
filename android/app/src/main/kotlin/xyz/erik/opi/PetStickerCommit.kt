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
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetStickerAssets
import java.io.File
import java.io.IOException

private const val TAG = "OpiPetSticker"

/**
 * 「自有」宠物形态 = **插入图片**，不提交任何 Unicode 文本（2026-10-18 用户裁决）。
 *
 * 图片走 `InputConnection.commitContent`；编辑器不接受（含 API < 25）时上屏
 * **独立文本标签** `[小欧·笑]` 并 toast —— 绝不退回笑脸 emoji（用户裁决）。
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
    val file = petStickerFile(mood) ?: return
    dropComposing(ic)
    val uri = try {
        FileProvider.getUriForFile(this, "$packageName.fileprovider", file)
    } catch (e: IllegalArgumentException) {
        // file_paths.xml 与 cacheDir/pet 对不上（配置错误）—— 不让 IME 崩，同样走文本标签。
        Log.w(TAG, "FileProvider 配置错误，回退文本标签：$e")
        commitPetStickerTextFallback(ic, mood)
        return
    }
    val info = InputContentInfoCompat(
        uri,
        ClipDescription("小欧表情", arrayOf("image/png")),
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
        Log.i(TAG, "pet sticker inserted: ${file.name}")
        return
    }
    Log.w(TAG, "editor rejected image/png, fallback to text tag: ${file.name}")
    commitPetStickerTextFallback(ic, mood)
}

/**
 * assets/pet → cacheDir/pet（存在且非空则跳过，供 FileProvider 暴露读权限）。
 * 0 字节 = 上次拷贝被中断留下的残骸；`exists()` 为真但图是坏的，必须重拷。
 * 缺资产时 toast 并返回 null。
 */
private fun OpiImeService.petStickerFile(mood: PetMood): File? {
    val path = PetEmoji.assetPath(mood)
    val file = File(File(cacheDir, "pet").apply { mkdirs() }, PetStickerAssets.stickerFile(mood))
    if (file.length() > 0) return file
    return try {
        assets.open(path).use { input -> file.outputStream().use { input.copyTo(it) } }
        file
    } catch (e: IOException) {
        Log.w(TAG, "pet sticker asset missing: $path", e)
        toast("小欧表情暂不可用")
        null
    }
}

/**
 * 编辑器不支持插入图片时的回退：上屏**独立文本标签** `[小欧·笑]`，绝不退回笑脸 emoji。
 *
 * 不走剪贴板图片：FileProvider 的读权限授不到任意粘贴方，会出现「提示已复制却粘不出来」。
 */
private fun OpiImeService.commitPetStickerTextFallback(ic: InputConnection, mood: PetMood) {
    dropComposing(ic)
    ic.commitText("[小欧·${PetEmoji.label(mood)}]", 1)
    toast("当前应用不支持插入图片，已用文字标签")
}

private fun OpiImeService.toast(text: String) {
    Toast.makeText(this, text, Toast.LENGTH_SHORT).show()
}
