// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import xyz.erik.opi.BuildConfig

/**
 * 设置页「关于」一节（2026-10-15，用户裁决「移动端：增加关于按钮，主要是项目简介、版本、
 * 版权、项目地址、博客地址」）。形式与 [DictSection] / [WubiSection] 两节一致：单独文件
 * + `SettingsScreen` 里一行调用（别把那页顶破 500 行）。
 *
 * 文案**不自编**，每条都有落点（要改就去改源头，别只改这里）：
 * · 简介 —— `README.md` 的「🎯 项目定位」段与二级标题（两句都逐字抄，去掉 markdown 加粗）；
 * · 版本 —— `android/app/build.gradle.kts` 的 `versionName`，经 `BuildConfig.VERSION_NAME`
 *   读到（**不写死字面量**，理由见 [AboutSectionTest] 的头注）；
 * · 版权 —— README「📄 许可证」段 + 各源文件 SPDX 头的 `2026 erik.xyz`；
 *   2026-10-15 用户裁决「合并：只留关于节」⇒ 设置页尾那行独立的版权行（2026-10-09
 *   裁决加的）已删，**版权只在本节出现**；这里是它唯一的落点。
 * · 项目地址 —— `scripts/nfpm.yaml` 的 `homepage`（README 里**没有**仓库地址）；
 * · 博客地址 —— 用户 2026-10-15 给的原话（`https://erik.xyz`）。
 *
 * ⚠️ **引擎没有版本出口**：JNI 注册表（`crates/opi-ffi/src/jni.rs` 的 `RegisterNatives`）
 * 与 C ABI（`crates/opi-ffi/src/cabi.rs`）里都没有 version 一项 —— 所以这里只报 App 版本，
 * 并把「读不到」如实写出来，不编一个引擎版本。
 */
internal const val ABOUT_INTRO =
    "小欧输入法（opi）是一款回归本质、人人可用的多端输入法。" +
        "Open People Input 开放、纯粹、跨端，" +
        "致力于为每一位用户提供不被干扰、不被窥探、不被绑架的输入体验。"

/** 项目地址（同 `scripts/nfpm.yaml` 的 `homepage`；锁步门禁 `AboutSectionTest`）。 */
internal const val ABOUT_PROJECT_URL = "https://github.com/erikwang2013/opi"

/** 博客地址（用户 2026-10-15 给定）。 */
internal const val ABOUT_BLOG_URL = "https://erik.xyz"

/** 官网主页（用户 2026-10-20 给定）。 */
internal const val ABOUT_HOME_URL = "https://opi.erik.xyz"

@Composable
internal fun AboutSection(toast: (String) -> Unit) {
    HorizontalDivider()
    ListItem(
        headlineContent = { Text("关于") },
        supportingContent = { Text(ABOUT_INTRO) },
    )
    ListItem(
        headlineContent = { Text("版本") },
        supportingContent = {
            Column {
                Text("v${BuildConfig.VERSION_NAME}")
                Text(
                    "引擎随本 App 一起打包（libopi_ffi.so），没有独立的版本出口，App 内读不到" +
                        " —— 只有上面的 App 版本。",
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        },
    )
    ListItem(
        headlineContent = { Text("版权") },
        supportingContent = {
            Text("© 2026 erik.xyz · 代码按 MIT 许可（全文见仓库 LICENSE）；词库数据与代码分开授权")
        },
    )
    ListItem(
        headlineContent = { Text("表情数据") },
        supportingContent = {
            Text("Emoji 取自 OpenMoji（CC BY-SA 4.0）；颜文字取自 rime_kaomoji_dict（MIT）；小欧贴纸/图标为项目专有资产，商业使用需授权")
        },
    )
    LinkItem("主页地址", ABOUT_HOME_URL, toast)
    LinkItem("项目地址", ABOUT_PROJECT_URL, toast)
    LinkItem("博客地址", ABOUT_BLOG_URL, toast)
}

/**
 * 一行可点的外链：整行即按钮（`role = Button` ⇒ 读屏会报「按钮」+ 行内容），
 * `onClickLabel` 给动作一个名字。行内仍把**完整地址**写出来 —— 读屏读得到，
 * 拿不到浏览器/网络时用户也看得见它是什么。
 */
@Composable
private fun LinkItem(label: String, url: String, toast: (String) -> Unit) {
    val context = LocalContext.current
    ListItem(
        modifier = Modifier.clickable(role = Role.Button, onClickLabel = "打开链接") {
            openExternal(context, url, toast)
        },
        headlineContent = { Text(label) },
        supportingContent = { Text(url) },
    )
}

/**
 * 交给系统「打开链接」（浏览器或能接该 scheme 的 App）。设备上没有任何能接的应用时
 * `startActivity` 抛 [ActivityNotFoundException] —— 吞掉并给可见反馈（Snackbar），
 * 别让设置页崩在「点了下地址」上。
 *
 * ⚠️ **不用 `resolveActivity` 预判**：API 30+ 的包可见性会让它在有浏览器时也返回 null
 * （那就成了「明明能开却说开不了」），而 `startActivity` 本身不受该限制。
 * 这里的 context 是 Activity（`SettingsActivity` 直接 setContent），无需 NEW_TASK。
 */
private fun openExternal(context: Context, url: String, toast: (String) -> Unit) {
    try {
        context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
    } catch (_: ActivityNotFoundException) {
        toast("没有能打开链接的应用：$url")
    }
}
