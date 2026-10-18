// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「自有」表情子页（2026-10-18 用户裁决：自有是独立顶级分类，点小欧形态插入**图片**）。
//!
//! 桌面进程没有 TSF / 命名管道的图片插入面（Android 那样直接往输入框塞图做不到），
//! 所以这里走**剪贴板回退**：点一格 = 把 `shared/pet/stickers-gif/<mood>.gif` 的原始
//! 字节优先放进系统剪贴板（GIF 通道），同时保留 PNG 的 [BufferedImage] 兜底；粘贴方
//! 不支持 GIF 时仍能拿到 PNG。窗内提示「可 Ctrl+V 粘贴」。
//! **绝不**走 `opi-ctl insert` 文本通道（那是文字上屏，粘不出图）。
//!
//! 文件名的唯一映射在 `shared/pet/PetStickerAssets.kt`（与 Android 共用）；
//! 资源路径 `/pet/`、`/pet-gif/` 由 `desktop/build.gradle.kts` 的 `copyPetStickers`
//! 在构建期从 `shared/pet/` 拷进 resources。
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
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import xyz.erik.opi.pet.PetFontResources
import xyz.erik.opi.pet.petMotion
import xyz.erik.opi.pet.rememberPetMotionPhase
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
import java.io.ByteArrayInputStream
import javax.imageio.ImageIO

/** 自有网格：4 列 × 3 行 = 12 个形态，一页放下（没有分页器/分类条）。 */
internal const val STICKER_COLUMNS = 4
internal const val STICKER_ROWS = 3

/** 复制图片的出口。测试注入假实现；生产用 [SystemImageClipboard]。 */
fun interface ImageClipboard {
    /**
     * [gif] 非空时优先以 `image/gif` 提供；返回 false = 系统剪贴板不可用
     * （headless / 被别的进程占住），不是「图坏了」。
     */
    fun copy(image: BufferedImage, gif: ByteArray?): Boolean
}

/** GIF 的 AWT 剪贴板口味；粘贴方按 `image/gif` 取原始字节流。 */
private val gifFlavor = DataFlavor("image/gif;class=java.io.InputStream")

/** AWT 系统剪贴板：GIF 在前、PNG 在后（同一份数据带两个 flavor，由粘贴方挑）。 */
val SystemImageClipboard = ImageClipboard { image, gif ->
    val bytes = gif
    try {
        java.awt.Toolkit.getDefaultToolkit().systemClipboard.setContents(
            object : Transferable {
                override fun getTransferDataFlavors(): Array<DataFlavor> =
                    if (bytes != null) arrayOf(gifFlavor, DataFlavor.imageFlavor)
                    else arrayOf(DataFlavor.imageFlavor)

                override fun isDataFlavorSupported(flavor: DataFlavor): Boolean =
                    flavor == DataFlavor.imageFlavor || (bytes != null && flavor == gifFlavor)

                override fun getTransferData(flavor: DataFlavor): Any = when {
                    flavor == DataFlavor.imageFlavor -> image
                    bytes != null && flavor == gifFlavor -> ByteArrayInputStream(bytes)
                    else -> throw UnsupportedFlavorException(flavor)
                }
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

/** 从 classpath 读 `/pet-gif/<file>.gif`；缺失 → null（剪贴板只剩 PNG）。 */
internal fun loadPetStickerGif(file: String): ByteArray? =
    PetStickerAssets::class.java.getResourceAsStream("/pet-gif/$file")?.use { it.readBytes() }

/** 一次自有格点击的结果：给用户看的提示 + 是否成功（决定提示行颜色）。 */
internal data class StickerCopyResult(val notice: String, val ok: Boolean)

/**
 * 自有格的点击语义（UI 与测试共用这一支）：GIF + PNG 一起放剪贴板，**不返回任何文本**。
 *
 * [load] / [loadGif] 可注入：测试不依赖真资产；生产缺省读 classpath。GIF 缺失或损坏
 * 只降级成 PNG，不算整体失败（PNG 仍是可用图）。
 */
internal fun copyPetSticker(
    form: PetEmojiMap.Form,
    clipboard: ImageClipboard,
    load: (String) -> BufferedImage? = ::loadPetSticker,
    loadGif: (String) -> ByteArray? = ::loadPetStickerGif,
): StickerCopyResult {
    val file = PetStickerAssets.stickerFile(form.mood)
    val image = load(file) ?: return StickerCopyResult("小欧表情图片缺失（/pet/$file）", false)
    val gif = loadGif(PetStickerAssets.gifFile(form.mood))
    return if (clipboard.copy(image, gif)) {
        StickerCopyResult("小欧表情已复制，可 Ctrl+V 粘贴", true)
    } else {
        StickerCopyResult("复制失败：系统剪贴板不可用", false)
    }
}

/**
 * 自有子页的本地搜索（12 个形态的标签/内置短码子串；与颜文字搜索同一条「数据在
 * 进程里」的路）。短码来自 classpath 的 `pet-font/shortcodes.tsv`。
 */
internal fun petStickerSearch(query: String): List<PetEmojiMap.Form> {
    val q = query.trim()
    if (q.isEmpty()) return PetEmojiMap.forms
    val byShortcode = PetFontResources.shortcodes()
        ?.filter { it.shortcode.contains(q, ignoreCase = true) }
        ?.map { it.file }
        ?.toSet()
        .orEmpty()
    return PetEmojiMap.forms.filter {
        it.label.contains(q, ignoreCase = true) ||
            PetStickerAssets.stickerFile(it.mood) in byShortcode
    }
}

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
    // 内置 map.tsv 是形态标签的真源；读不到才用 PetEmojiMap 的标签。
    val labels = remember { PetFontResources.map()?.associate { it.mood to it.label }.orEmpty() }
    // Compose Desktop 的 Image 不播 GIF：面板用 PNG + 共享相位动效；点击复制的是 GIF。
    val phase = rememberPetMotionPhase()
    PagedGrid(columns = STICKER_COLUMNS, rows = STICKER_ROWS, pageStart = 0, total = forms.size) { i ->
        val form = forms[i]
        val label = labels[form.mood.name] ?: form.label
        PetStickerCell(form, label, images[form.mood], phase, Modifier.weight(1f), onCopy)
    }
}

@Composable
private fun PetStickerCell(
    form: PetEmojiMap.Form,
    label: String,
    image: ImageBitmap?,
    phase: State<Float>,
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
            .semantics { contentDescription = "小欧$label，复制图片表情" },
        contentAlignment = Alignment.Center,
    ) {
        if (image != null) {
            Image(
                bitmap = image,
                contentDescription = null,
                contentScale = ContentScale.Fit,
                modifier = Modifier.fillMaxHeight().padding(6.dp).petMotion(form.mood, phase),
            )
        } else {
            Text(form.label, fontSize = 12.sp, color = SymDim)
        }
    }
}
