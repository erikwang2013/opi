// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「关于」一节的**锁步门禁**（2026-10-15）。文案本身是抄给用户读的（源头见
//! `AboutSection.kt` 头注），机器钉住的是两条**会静默漂、漂了不报错**的：
//!
//! · **版本串必须经 `BuildConfig.VERSION_NAME`**（= `build.gradle.kts` 的 `versionName`），
//!   不许在页面里写死字面量 —— 本仓已有一次实案：desktop 的 SettingsWindow 把版本写死
//!   成当时的 1.4.0，之后发布到 1.6.0，那行就成了假话（见该文件里 OPI_VERSION 的注释）。
//!   实现侧因此扫源码：既要求出现 `BuildConfig.VERSION_NAME`，又要求整个文件里
//!   **没有** `x.y.z` 形态的字面量（注释也算 —— 会漂的数字本来就不该写进这里）。
//! · **项目地址必须与打包元数据同源**：`scripts/nfpm.yaml` 的 `homepage`
//!   （README 里没有仓库地址，打包元数据才是那个会跟着仓库名一起改的落点）。
//!
//! 测试工作目录 = `android/app`（同 FuzzyPairsLockstepTest 的口径）。

package xyz.erik.opi.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.BuildConfig
import java.io.File

class AboutSectionTest {

    @Test
    fun versionComesFromBuildConfigNotFromALiteral() {
        val src = File("src/main/kotlin/xyz/erik/opi/settings/AboutSection.kt").readText()
        assertTrue(
            "版本必须取自 BuildConfig.VERSION_NAME（写死的字面量会随发布漂）",
            src.contains("BuildConfig.VERSION_NAME"),
        )
        val literal = Regex("""\d+\.\d+\.\d+""").find(src)?.value
        assertNull("「关于」里不许出现写死的版本号，改成读 BuildConfig：$literal", literal)
        // buildConfig 没打开时这行根本编译不过；能跑到这里就说明版本文本是真生成的。
        assertTrue(
            "BuildConfig.VERSION_NAME 应是 x.y.z 形态（现在是 ${BuildConfig.VERSION_NAME}）",
            BuildConfig.VERSION_NAME.matches(Regex("""\d+\.\d+\.\d+.*""")),
        )
    }

    @Test
    fun projectUrlMatchesPackagingHomepage() {
        val yaml = File("../../scripts/nfpm.yaml")
        assertTrue("打包元数据不在了（搬家了？）：$yaml", yaml.exists())
        val homepage = yaml.readText().lineSequence()
            .first { it.startsWith("homepage:") }
            .substringAfter("homepage:").trim()
        assertEquals("Android「关于」的项目地址必须与打包 homepage 同源", homepage, ABOUT_PROJECT_URL)
    }

    /**
     * **关于面板**（2026-10-15 用户裁决「关于给移动端键盘功能区加个入口」）。
     *
     * 两条会静默漂的：① 面板里**不许另抄一份文案/版本**（版本若写死，下次发布就成假话
     * —— 与设置页那条同一个病）；② 工具条 → 面板这条**接线**（图标少一枚、分支名字打错，
     * 单测都看不见，编译也照样过）。
     */
    @Test
    fun aboutPanelReusesSourcesAndIsWiredToTheToolbar() {
        val panel = File("src/main/kotlin/xyz/erik/opi/ime/AboutPanel.kt").readText()
        assertTrue("面板必须读 BuildConfig.VERSION_NAME", panel.contains("BuildConfig.VERSION_NAME"))
        assertNull(
            "关于面板里不许出现写死的版本号：${Regex("""\d+\.\d+\.\d+""").find(panel)?.value}",
            Regex("""\d+\.\d+\.\d+""").find(panel)?.value,
        )
        for (c in listOf("ABOUT_INTRO", "ABOUT_HOME_URL", "ABOUT_PROJECT_URL", "ABOUT_BLOG_URL")) {
            assertTrue("面板必须复用设置侧常量 $c（文案单一来源）", panel.contains(c))
        }
        val toolbar = File("src/main/kotlin/xyz/erik/opi/ime/ImeToolbar.kt").readText()
        assertTrue("工具条上要有「关于」入口", toolbar.contains("OpiIcons.Info, \"关于\""))
        val screen = File("src/main/kotlin/xyz/erik/opi/ime/ImeScreen.kt").readText()
        assertTrue("工具条回调要接到 state::openAbout", screen.contains("onOpenAbout = state::openAbout"))
        assertTrue("View.ABOUT 要分发到 AboutPanel", screen.contains("ImeState.View.ABOUT -> AboutPanel("))
        val state = File("src/main/kotlin/xyz/erik/opi/ime/ImeState.kt").readText()
        assertTrue("ImeState 要有 openAbout()", state.contains("fun openAbout()"))
        // 枚举成员**解析出来查**，别钉整行字面串：2026-10-16 加 CANDIDATES_TILED 时
        // 那种「SYMBOL_PAGE, ABOUT }」的整串断言红过一次 —— 它测的是源码文本，不是
        // 「ABOUT 在不在」这个行为（同本仓文本门禁的老坑）。
        val viewMembers = Regex("""enum class View \{([^}]*)\}""")
            .find(state)?.groupValues?.get(1)
            ?: error("ImeState.kt 里找不到 View 枚举")
        assertTrue("View 枚举要有 ABOUT（成员：$viewMembers）", viewMembers.contains("ABOUT"))
    }
}
