// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 2026-10-18：自有表情改插入 PNG（用户裁决）。OpiImeService 已 494 行（500 行门禁），
// 图片通道整体拆在这里 —— 扩展函数，读的仍是 service 的成员（同 ComposingText.kt 先例）。
// 2026-10-20 用户裁决：commitContent 被宿主拒绝时加**剪贴板图片回退**（复制图片 + 提示长按粘贴），并同时上屏文字标签保证至少有一个可见结果。

package xyz.erik.opi

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
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

/** 一次贴纸上屏的结局（纯决策，见 [petStickerOutcome]；JVM 测试钉住顺序）。 */
internal enum class PetStickerOutcome { INSERTED, COPIED, TEXT_FALLBACK }

/**
 * 图片通道失败后的回退决策（纯函数）：图片插进去 > 图片复制到剪贴板 > 文字标签。
 * 三者互斥；「已复制」绝不能被报成「已插入」。
 */
internal fun petStickerOutcome(imageInserted: Boolean, clipboardCopied: Boolean): PetStickerOutcome =
    when {
        imageInserted -> PetStickerOutcome.INSERTED
        clipboardCopied -> PetStickerOutcome.COPIED
        else -> PetStickerOutcome.TEXT_FALLBACK
    }

/** 各结局给用户看的话；[PetStickerOutcome.INSERTED] 不需要提示（图已经出现在输入框里）。 */
internal fun petStickerNotice(outcome: PetStickerOutcome): String? = when (outcome) {
    PetStickerOutcome.INSERTED -> null
    PetStickerOutcome.COPIED -> "表情图片已复制，请在输入框长按选择「粘贴」"
    PetStickerOutcome.TEXT_FALLBACK -> "当前应用不支持插入图片，已用文字标签"
}

/** 图片通道的尝试顺序：PNG → GIF（PNG 是各宿主最认的格式；GIF 只是动图加分项）。 */
internal fun petStickerImageOrder(gif: File?, png: File?): List<Pair<String, File>> =
    listOfNotNull(png?.let { "image/png" to it }, gif?.let { "image/gif" to it })

/**
 * 「自有」宠物形态 = **插入图片**，不提交任何 Unicode 文本（2026-10-18 用户裁决）。
 *
 * 图片走 `InputConnection.commitContent`，格式优先级（2026-10-20）：**PNG → GIF**；
 * 宿主两个都拒绝时（2026-10-20）把图放进系统剪贴板并提示长按粘贴，**同时**上屏
 * `[小欧·笑]` 这样的独立标签兜底 —— 目标 app 读不读得到 URI 在 IME 侧观测不到，
 * 标签保证「至少有一个可见结果」，绝不静默。剪贴板也写不进去时只上屏标签。
 * **不再退回 Unicode emoji**（2026-10-19 用户裁决：emoji 与自有表情分不清）。
 * GIF 是同一张 PNG 的轻微弹跳/缩放动画，见 `shared/pet/stickers-gif/`。
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
    // PNG 优先（2026-10-20）：宿主最认的格式；GIF 是动图加分项，PNG 被拒再试。
    val gif = petStickerFile(PetEmoji.gifAssetPath(mood), PetStickerAssets.gifFile(mood))
    val png = petStickerFile(PetEmoji.assetPath(mood), PetStickerAssets.stickerFile(mood))
    val images = petStickerImageOrder(gif, png)
    if (images.isEmpty()) {
        toast("小欧表情暂不可用")
        return
    }
    dropComposing(ic)
    for ((mime, file) in images) {
        if (commitPetStickerImage(ic, file, mime)) return
    }
    // 宿主不收 commitContent：剪贴板兜底（2026-10-20）。同样 PNG 优先。
    val copied = copyPetStickerToClipboard(images.first().second)
    val outcome = petStickerOutcome(imageInserted = false, clipboardCopied = copied)
    Log.w(TAG, "editor rejected image commit, outcome=$outcome: ${mood.name}")
    // 剪贴板成功也上屏标签：目标 app 能否读 URI 观测不到，标签是「至少不静默」的兜底。
    commitPetStickerTextFallback(ic, mood)
    petStickerNotice(outcome)?.let { toast(it) }
}

/**
 * 图片放进系统剪贴板（`content://` URI）。返回 false = 剪贴板写不进去（无此服务/
 * 被 OEM 限制/FileProvider 配置错），调用方回退文字标签。
 *
 * FileProvider 的 manifest 已开 `grantUriPermissions`；粘贴时系统会按需授予读权限，
 * 这里再按当前编辑器包名显式授一次 —— 表情图不含隐私数据，多给一份读权限没有代价，
 * 换来旧 OEM 上「已复制却粘不出」的窗口更小。
 */
private fun OpiImeService.copyPetStickerToClipboard(file: File): Boolean = try {
    val uri = FileProvider.getUriForFile(this, "$packageName.fileprovider", file)
    currentInputEditorInfo?.packageName
        ?.takeIf { it.isNotEmpty() && it != packageName }
        ?.let { grantUriPermission(it, uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
    val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    cm.setPrimaryClip(ClipData.newUri(contentResolver, "小欧表情", uri))
    Log.i(TAG, "pet sticker copied to clipboard: ${file.name}")
    true
} catch (e: Exception) {
    // 回退路径不允许把 IME 带崩：任何失败都只是「这一档没成」。
    Log.w(TAG, "pet sticker clipboard copy failed: $e")
    false
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
 * commitContent 失败后的可见兜底：上屏**独立文本标签** `[小欧·笑]`，绝不退回笑脸 emoji
 * （2026-10-19 用户裁决：emoji 和自有表情在输入框里分不清）。剪贴板成功时也调用它 ——
 * 目标 app 能否读 URI 在 IME 侧观测不到；本函数不提示，文案见 [petStickerNotice]（单一口径）。
 */
private fun OpiImeService.commitPetStickerTextFallback(ic: InputConnection, mood: PetMood) {
    dropComposing(ic)
    // 标签本身已带「小欧·」前缀（PetEmojiMap 是唯一源），这里只加方括号。
    ic.commitText("[${PetEmoji.label(mood)}]", 1)
}

private fun OpiImeService.toast(text: String) {
    Toast.makeText(this, text, Toast.LENGTH_SHORT).show()
}
