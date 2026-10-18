// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import android.content.Intent
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.engine.UserWordStore
import java.io.File

/**
 * 覆盖安装自动重载的**配置面门禁**。
 *
 * 真机上「kill 后系统会不会重新绑定 IME」JVM 测不了（见 docs/install/android.md §3.4）；
 * 这里钉的是三件会静默失效的事：manifest 声明、只认覆盖安装广播、以及退出延迟必须盖过
 * 用户词防抖窗口（否则刚选的词会随进程一起消失）。
 */
class PackageReplacedConfigTest {

    @Test
    fun manifestDeclaresPackageReplacedReceiver() {
        val manifest = File("src/main/AndroidManifest.xml").readText()
        for (needle in listOf(
            """android:name=".PackageReplacedReceiver"""",
            """android:exported="false"""",
            """android:name="android.intent.action.MY_PACKAGE_REPLACED"""",
        )) {
            assertTrue("manifest 缺：$needle", manifest.contains(needle))
        }
    }

    @Test
    fun onlyThePackageReplacedActionIsHandled() {
        assertTrue(PackageReplacedReceiver.handles(Intent.ACTION_MY_PACKAGE_REPLACED))
        // 普通应用包的 PACKAGE_REPLACED 与本应用无关，不得触发退出
        assertFalse(PackageReplacedReceiver.handles("android.intent.action.PACKAGE_REPLACED"))
        assertFalse(PackageReplacedReceiver.handles(null))
    }

    @Test
    fun exitWaitsOutTheUserWordDebounceWindow() {
        assertTrue(
            "退出延迟必须大于用户词防抖窗口，否则防抖中的选词落不了盘",
            PackageReplacedReceiver.EXIT_DELAY_MS > UserWordStore.SAVE_DEBOUNCE_MS,
        )
    }
}
