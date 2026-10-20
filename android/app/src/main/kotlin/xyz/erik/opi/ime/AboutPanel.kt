// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.BuildConfig
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.settings.ABOUT_BLOG_URL
import xyz.erik.opi.settings.ABOUT_HOME_URL
import xyz.erik.opi.settings.ABOUT_INTRO
import xyz.erik.opi.settings.ABOUT_PROJECT_URL

/**
 * **关于面板**（2026-10-15 用户裁决「关于给移动端键盘功能区加个入口」）。
 *
 * 为什么另开面板而不是跳设置页：设置页里那一节（`settings/AboutSection.kt`，同日落地）
 * 要先进设置、再滑到底 —— 用户要的是**功能区里的一个入口**，点开即看。文案与常量
 * **复用同一份**（`ABOUT_INTRO` / `ABOUT_PROJECT_URL` / `ABOUT_BLOG_URL`，
 * 版本经 `BuildConfig.VERSION_NAME`）—— 面板里一个字的正文都不自编，改文案去改源头。
 *
 * 形态与皮肤/输入模式面板同一套（左上返回箭头 + 标题行，[ImeState.backToLetters]）。
 * 内容比键区高 ⇒ 本面板**自己可滚动**（与设置页那个滚动缺陷同族的坑，别省这一步）。
 */
@Composable
fun AboutPanel(onClose: () -> Unit, modifier: Modifier = Modifier) {
    val p = LocalKeyboardPalette.current
    val context = LocalContext.current
    // 打不开链接时的就地反馈（IME 里没有 Snackbar 宿主；设置页那边走 toast，
    // 这里就把话写在面板里，别静默）。
    var note by remember { mutableStateOf("") }
    Column(modifier = modifier.fillMaxSize().background(p.panelBg)) {
        PanelHeader("关于", "", onClose)
        Column(
            modifier = Modifier.fillMaxWidth().weight(1f)
                .verticalScroll(rememberScrollState())
                .padding(start = 14.dp, end = 14.dp, bottom = 10.dp),
        ) {
            Para(ABOUT_INTRO)
            Head("版本")
            Para("v${BuildConfig.VERSION_NAME}")
            Head("版权")
            Para("© 2026 erik.xyz · 代码按 MIT 许可（全文见仓库 LICENSE）；词库数据与代码分开授权")
            Head("表情数据")
            Para("Emoji 取自 OpenMoji（CC BY-SA 4.0）；颜文字取自 rime_kaomoji_dict（MIT）；小欧贴纸/图标为项目专有资产，商业使用需授权")
            Head("主页地址")
            Link(ABOUT_HOME_URL) { note = openExternal(context, ABOUT_HOME_URL) }
            Head("项目地址")
            Link(ABOUT_PROJECT_URL) { note = openExternal(context, ABOUT_PROJECT_URL) }
            Head("博客地址")
            Link(ABOUT_BLOG_URL) { note = openExternal(context, ABOUT_BLOG_URL) }
            if (note.isNotEmpty()) {
                Text(note, fontSize = 11.sp, color = p.accent, modifier = Modifier.padding(top = 6.dp))
            }
        }
    }
}

/** 小标题（与设置页那节的 headline 同义，这里只为省纵向空间用一行小字）。 */
@Composable
private fun Head(text: String) {
    val p = LocalKeyboardPalette.current
    Text(
        text,
        fontSize = 12.sp,
        fontWeight = FontWeight.Bold,
        color = p.panelFg,
        modifier = Modifier.padding(top = 8.dp),
    )
}

/** 正文一行。 */
@Composable
private fun Para(text: String) {
    val p = LocalKeyboardPalette.current
    Text(text, fontSize = 12.sp, color = p.panelFg)
}

/** 可点的外链：整行即按钮，行内把**完整地址**写出来（读屏读得到、开不了也看得见）。 */
@Composable
private fun Link(url: String, onClick: () -> Unit) {
    val p = LocalKeyboardPalette.current
    Text(
        url,
        fontSize = 12.sp,
        color = p.accent,
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(6.dp))
            .clickable(role = Role.Button, onClickLabel = "打开链接") { onClick() }
            .padding(vertical = 4.dp),
    )
    Spacer(Modifier.height(2.dp))
}

/**
 * 交给系统打开链接；返回**给用户看的一句反馈**（空串 = 成功）。
 *
 * ⚠️ 与设置页那份（`settings/AboutSection.kt` 的 `openExternal`）有一处**必须不同**：
 * 这里的 context 是 **Service**（IME），从 Service 起 Activity 要 `NEW_TASK`，
 * 少了它就抛 `AndroidRuntimeException`（不是 ActivityNotFound，下面那个 catch 接不住）。
 */
private fun openExternal(context: Context, url: String): String = try {
    context.startActivity(
        Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
    )
    ""
} catch (_: ActivityNotFoundException) {
    "没有能打开链接的应用：$url"
}
