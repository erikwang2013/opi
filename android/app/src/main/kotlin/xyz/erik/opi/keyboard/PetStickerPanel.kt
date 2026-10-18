// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import android.content.res.AssetManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.Log
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood
import java.io.IOException

/** 解码目标边长（px）：格子画 40dp，density 2.75 下约 110px，128 足够且省 12 张图的内存。 */
private const val PET_STICKER_MAX_PX = 128

/**
 * 「自有」子页：12 个小欧形态（PNG 表情图，asset 路径见 [PetEmoji.assetPath]）。
 *
 * 点击走 [onPick]（图片插入通道），**不经过**文本提交与最近使用 —— 自有与 Emoji
 * 是独立顶级分类（2026-10-18 用户裁决）。资产缺失时退回矢量 [OpiPet] 占位，
 * 面板不出现空白格；真点击时服务侧会 toast「暂不可用」。
 */
@Composable
internal fun PetStickerPanel(onPick: (PetMood) -> Unit) {
    val forms = remember { PetEmoji.forms }
    val assets = LocalContext.current.assets
    val loaded = remember { mutableStateMapOf<PetMood, ImageBitmap>() }
    // 解码放 IO：12 张 PNG 若在首帧同步解，打开自有页会卡一下。
    LaunchedEffect(Unit) {
        val images = withContext(Dispatchers.IO) {
            forms.mapNotNull { f ->
                decodeSticker(assets, PetEmoji.assetPath(f.mood))?.let { f.mood to it.asImageBitmap() }
            }
        }
        for ((mood, image) in images) loaded[mood] = image
    }
    if (forms.isEmpty()) {
        Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("没有小欧表情数据", fontSize = 14.sp, color = LocalKeyboardPalette.current.barDimFg)
        }
        return
    }
    LazyVerticalGrid(columns = GridCells.Fixed(6), modifier = Modifier.fillMaxSize()) {
        items(forms.size) { i ->
            val form = forms[i]
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(4.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .clickable { onPick(form.mood) }
                    .semantics { contentDescription = "小欧${form.label}，插入图片表情" },
                contentAlignment = Alignment.Center,
            ) {
                val image = loaded[form.mood]
                if (image != null) {
                    Image(
                        bitmap = image,
                        contentDescription = null, // 读屏走格子 Box 的语义
                        modifier = Modifier.size(40.dp),
                        contentScale = ContentScale.Fit,
                    )
                } else {
                    OpiPet(mood = form.mood, size = 40.dp)
                }
            }
        }
    }
}

/** 按目标边长降采样解码；缺资产/坏图返回 null（调用方退回矢量占位）。 */
private fun decodeSticker(assets: AssetManager, path: String): Bitmap? = try {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    assets.open(path).use { BitmapFactory.decodeStream(it, null, bounds) }
    var sample = 1
    while (bounds.outWidth / (sample * 2) >= PET_STICKER_MAX_PX &&
        bounds.outHeight / (sample * 2) >= PET_STICKER_MAX_PX
    ) {
        sample *= 2
    }
    val options = BitmapFactory.Options().apply { inSampleSize = sample }
    assets.open(path).use { BitmapFactory.decodeStream(it, null, options) }
} catch (e: IOException) {
    Log.w("OpiPetSticker", "pet sticker asset missing: $path", e)
    null
}
