// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「自有」表情子页（2026-10-18 用户裁决：自有是独立顶级分类，点小欧形态插入**图片**）。
//!
//! 桌面进程没有 TSF / 命名管道的图片插入面（Android 那样直接往输入框塞图做不到），
//! 所以这里走**剪贴板回退**：点一格 = 把 `shared/pet/stickers/<mood>.png` 的
//! [BufferedImage] 放进系统剪贴板 + 在窗内提示「可 Ctrl+V 粘贴」。
//! **绝不**走 `opi-ctl insert` 文本通道（那是文字上屏，粘不出图）。
//!
//! PNG 文件名的唯一映射在 `shared/pet/PetStickerAssets.kt`（与 Android 共用）；
//! 资源路径 `/pet/<file>.png` 由 `desktop/build.gradle.kts` 的 `copyPetStickers`
//! 在构建期从 `shared/pet/stickers/` 拷进 resources。
//!
//! 剪贴板接口可注入（[ImageClipboard]）：纯 JVM 测试用假实现，不碰真系统剪贴板。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.padding
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.pet.PetEmojiMap
import xyz.erik.opi.pet.PetStickerAssets
import java.awt.datatransfer.DataFlavor
import java.awt.datatransfer.Transferable
import java.awt.datatransfer.UnsupportedFlavorException
import java.awt.image.BufferedImage
import javax.imageio.ImageIO

/** 自有网格：4 列 × 3 行 = 12 个形态，一页放下（没有分页器/分类条）。 */
internal const val STICKER_COLUMNS = 4
internal const val STICKER_ROWS = 3

/** 复制图片的出口。测试注入假实现；生产用 [SystemImageClipboard]。 */
fun interface ImageClipboard {
    /** 返回 false = 系统剪贴板不可用（headless / 被别的进程占住），不是「图坏了」。 */
    fun copy(image: BufferedImage): Boolean
}

/** AWT 系统剪贴板。只放 `imageFlavor` —— 粘贴目标是文档，不是文本通道。 */
val SystemImageClipboard = ImageClipboard { image ->
    try {
        java.awt.Toolkit.getDefaultToolkit().systemClipboard.setContents(
            object : Transferable {
                override fun getTransferDataFlavors(): Array<DataFlavor> = arrayOf(DataFlavor.imageFlavor)
                override fun isDataFlavorSupported(flavor: DataFlavor): Boolean =
                    flavor == DataFlavor.imageFlavor
                override fun getTransferData(flavor: DataFlavor): Any =
                    if (flavor == DataFlavor.imageFlavor) image
                    else throw UnsupportedFlavorException(flavor)
            },
            null, // owner = null：数据由 toolkit 守护，窗口关掉也还在剪贴板里
        )
        true
    } catch (e: IllegalStateException) {
        // Linux 上剪贴板被另一个进程持有（X11 选择所有权竞争）时的既有异常形态。
        false
    } catch (e: java.awt.HeadlessException) {
        false
    }
}

/** 从 classpath 读 `/pet/<file>.png`；缺文件 / 解不出 → null（页面显示缺图占位）。 */
internal fun loadPetSticker(file: String): BufferedImage? =
    PetStickerAssets::class.java.getResourceAsStream("/pet/$file")?.use { ImageIO.read(it) }

/** 一次自有格点击的结果：给用户看的提示 + 是否成功（决定提示行颜色）。 */
internal data class StickerCopyResult(val notice: String, val ok: Boolean)

/**
 * 自有格的点击语义（UI 与测试共用这一支）：复制 PNG 到剪贴板，**不返回任何文本**。
 *
 * [load] 可注入：测试不依赖真 PNG 资产；生产缺省走 [loadPetSticker]。
 */
internal fun copyPetSticker(
    form: PetEmojiMap.Form,
    clipboard: ImageClipboard,
    load: (String) -> BufferedImage? = ::loadPetSticker,
): StickerCopyResult {
    val file = PetStickerAssets.stickerFile(form.mood)
    val image = load(file) ?: return StickerCopyResult("小欧表情图片缺失（/pet/$file）", false)
    return if (clipboard.copy(image)) {
        StickerCopyResult("小欧表情已复制，可 Ctrl+V 粘贴", true)
    } else {
        StickerCopyResult("复制失败：系统剪贴板不可用", false)
    }
}

/** 自有子页的本地搜索（12 个形态的标签子串；与颜文字搜索同一条「数据在进程里」的路）。 */
internal fun petStickerSearch(query: String): List<PetEmojiMap.Form> =
    PetEmojiMap.forms.filter { it.label.contains(query.trim(), ignoreCase = true) }

/** 自有子页：4 列网格，一页装下 12 张宠物图片；点一格 = [onCopy]（不是文本上屏）。 */
@Composable
internal fun PetStickerPage(
    forms: List<PetEmojiMap.Form>,
    modifier: Modifier,
    onCopy: (PetEmojiMap.Form) -> Unit,
) {
    if (forms.isEmpty()) {
        CenteredHint("没有匹配的表情", modifier)
        return
    }
    // 12 张透明底小图，窗打开时解一次；解码失败的那格留占位文字（不整页崩）。
    val images = remember(forms) {
        forms.associate { form ->
            form.mood to loadPetSticker(PetStickerAssets.stickerFile(form.mood))?.toComposeImageBitmap()
        }
    }
    PagedGrid(columns = STICKER_COLUMNS, rows = STICKER_ROWS, pageStart = 0, total = forms.size) { i ->
        val form = forms[i]
        PetStickerCell(form, images[form.mood], Modifier.weight(1f), onCopy)
    }
}

@Composable
private fun PetStickerCell(
    form: PetEmojiMap.Form,
    image: ImageBitmap?,
    modifier: Modifier,
    onCopy: (PetEmojiMap.Form) -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    Box(
        modifier = modifier
            .fillMaxHeight()
            .padding(1.dp)
            .background(if (hovered) SymHover else Color.Transparent)
            .clickable(interactionSource = interaction, indication = null) { onCopy(form) }
            .semantics { contentDescription = "小欧${form.label}，复制图片表情" },
        contentAlignment = Alignment.Center,
    ) {
        if (image != null) {
            Image(
                bitmap = image,
                contentDescription = null,
                contentScale = ContentScale.Fit,
                modifier = Modifier.fillMaxHeight().padding(6.dp),
            )
        } else {
            Text(form.label, fontSize = 12.sp, color = SymDim)
        }
    }
}
