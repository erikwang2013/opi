// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import android.content.res.AssetManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.ImageDecoder
import android.graphics.Movie
import android.graphics.PixelFormat
import android.graphics.drawable.AnimatedImageDrawable
import android.graphics.drawable.Drawable
import android.os.Build
import android.os.SystemClock
import android.util.Log
import android.widget.ImageView
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
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetEmojiTables
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.petMotion
import xyz.erik.opi.pet.rememberPetMotionPhase
import java.io.IOException

/** 解码目标边长（px）：格子画 40dp，density 2.75 下约 110px，128 足够且省 12 张图的内存。 */
private const val PET_STICKER_MAX_PX = 128

/**
 * 「自有」子页：12 个小欧形态（GIF/PNG 表情图，asset 路径见 [PetEmoji.assetPath]）。
 *
 * 点击走 [onPick]（GIF → PNG 的图片插入通道），**不经过**文本提交与最近使用 ——
 * 自有与 Emoji 是独立顶级分类（2026-10-18 用户裁决）。格子里播**真 GIF**（API 28+
 * 的 ImageDecoder、21–27 的 `Movie` 兜底）；GIF 缺失才退回静态 PNG + 共享相位动效。
 * 资产缺失时退回矢量 [OpiPet] 占位，面板不出现空白格；真点击时服务侧会 toast「暂不可用」。
 */
@Composable
internal fun PetStickerPanel(onPick: (PetMood) -> Unit) {
    val forms = remember { PetEmoji.forms }
    val assets = LocalContext.current.assets
    // 内置 map.tsv 是形态标签的真源（PetEmojiMap 是兜底；两张表由生成器互校）。
    val labels = remember(assets) {
        PetEmojiTables.map(assets)?.associate { it.mood to it.label }.orEmpty()
    }
    val loaded = remember { mutableStateMapOf<PetMood, ImageBitmap>() }
    // 自有页优先显示**动图**（2026-10-19 用户裁决「用 gif 贴纸来显示」）：API 28+
    // 走 ImageDecoder，API 21–27 走 Movie 兜底；GIF 缺失才回退静态 PNG + 共享动效。
    val gifs = remember { mutableStateMapOf<PetMood, Drawable>() }
    // PNG 回退格仍用共享相位做轻微动效（有 GIF 的格子播真帧，不叠加）。
    val phase = rememberPetMotionPhase()
    // 解码放 IO：12 张 PNG 若在首帧同步解，打开自有页会卡一下。
    LaunchedEffect(Unit) {
        val decoded = withContext(Dispatchers.IO) {
            forms.mapNotNull { f ->
                val gif = decodeGif(assets, PetEmoji.gifAssetPath(f.mood))
                val png = decodeSticker(assets, PetEmoji.assetPath(f.mood))?.asImageBitmap()
                if (gif != null || png != null) Triple(f.mood, gif, png) else null
            }
        }
        for ((mood, gif, png) in decoded) {
            if (gif != null) gifs[mood] = gif
            if (png != null) loaded[mood] = png
        }
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
                    .semantics {
                        // 标签本身已带「小欧·」前缀（map.tsv / PetEmojiMap），别再加一次。
                        val label = labels[form.mood.name] ?: form.label
                        contentDescription = "$label，插入图片表情"
                    },
                contentAlignment = Alignment.Center,
            ) {
                val gif = gifs[form.mood]
                val image = loaded[form.mood]
                if (gif != null) {
                    AndroidView(
                        factory = { ctx -> ImageView(ctx).apply { scaleType = ImageView.ScaleType.FIT_CENTER } },
                        update = { view ->
                            view.setImageDrawable(gif)
                            startGif(gif)
                        },
                        // 格子滑出/换分类时停掉动画；Drawable 本体留在 gifs 里，滑回来能重启。
                        onRelease = { view ->
                            view.setImageDrawable(null)
                            stopGif(gif)
                        },
                        modifier = Modifier.size(40.dp).petMotion(form.mood, phase),
                    )
                } else if (image != null) {
                    Image(
                        bitmap = image,
                        contentDescription = null, // 读屏走格子 Box 的语义
                        modifier = Modifier.size(40.dp).petMotion(form.mood, phase),
                        contentScale = ContentScale.Fit,
                    )
                } else {
                    OpiPet(mood = form.mood, size = 40.dp)
                }
            }
        }
    }
}

/**
 * 解码动画 GIF：API 28+ 用 ImageDecoder；API 21–27 用 `android.graphics.Movie` 兜底
 * （minSdk = 21，低版本也必须显示动图）。缺资产/坏图返回 null，调用方退回 PNG/矢量。
 */
private fun decodeGif(assets: AssetManager, path: String): Drawable? = try {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        // 12 个 GIF 按原生 200×200 × ~10 帧解约 19MB；与 PNG 同一 128px 上限（资产均为方图）。
        ImageDecoder.decodeDrawable(ImageDecoder.createSource(assets, path)) { decoder, _, _ ->
            decoder.setTargetSize(PET_STICKER_MAX_PX, PET_STICKER_MAX_PX)
        }
    } else {
        assets.open(path).use { Movie.decodeStream(it) }?.let(::MovieDrawable)
    }
} catch (e: Exception) {
    Log.w("OpiPetSticker", "pet gif missing/unreadable: $path", e)
    null
}

/**
 * 启动 GIF。**先 setVisible(true, true)**：`setImageDrawable()` 不保证替我们调可见性，
 * 而 [MovieDrawable] 只有收到 setVisible(true) 才会 scheduleSelf 排下一帧（否则停在首帧）。
 * 单独的 API 28+ 分支是为了让低版本**不执行**引用 `AnimatedImageDrawable` 的代码。
 */
private fun startGif(drawable: Drawable) {
    drawable.setVisible(true, true)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        (drawable as? AnimatedImageDrawable)?.start()
    }
}

/** 停动画（不回收：Drawable 缓存在 `gifs` 里，格子滑回来还要重启）。 */
private fun stopGif(drawable: Drawable) {
    drawable.setVisible(false, false) // MovieDrawable 的重绘链在这里停
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        (drawable as? AnimatedImageDrawable)?.stop()
    }
}

/**
 * API 21–27 的最小 GIF Drawable：`Movie` 负责逐帧解码，这里按时间取帧。
 *
 * 只在可见时用 `scheduleSelf` 定帧重绘；ImageView 摘除/隐藏时系统会把它置为不可见，
 * 重绘链就停了（没有常驻线程，也不占主线程）。
 */
private class MovieDrawable(private val movie: Movie) : Drawable(), Runnable {

    private var startedMs = -1L

    override fun run() {
        if (!isVisible) return
        invalidateSelf()
        scheduleSelf(this, SystemClock.uptimeMillis() + FRAME_MS)
    }

    override fun draw(canvas: Canvas) {
        val w = movie.width()
        val h = movie.height()
        val b = getBounds()
        if (w <= 0 || h <= 0 || b.width() <= 0 || b.height() <= 0) return
        val now = SystemClock.uptimeMillis()
        if (startedMs < 0) startedMs = now
        movie.setTime(((now - startedMs) % movie.duration().coerceAtLeast(1)).toInt())
        val save = canvas.save()
        canvas.translate(b.left.toFloat(), b.top.toFloat())
        canvas.scale(b.width().toFloat() / w, b.height().toFloat() / h)
        movie.draw(canvas, 0f, 0f)
        canvas.restoreToCount(save)
    }

    override fun setAlpha(alpha: Int) = Unit // Movie 不支持逐帧 alpha/滤镜
    override fun setColorFilter(colorFilter: ColorFilter?) = Unit
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT

    override fun setVisible(visible: Boolean, restart: Boolean): Boolean {
        val changed = super.setVisible(visible, restart)
        unscheduleSelf(this)
        if (visible) {
            if (restart) startedMs = -1
            scheduleSelf(this, SystemClock.uptimeMillis())
        }
        return changed
    }

    private companion object {
        /** 30fps 足够（GIF 本身 90ms/帧），比 16ms 省电。 */
        const val FRAME_MS = 33L
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
