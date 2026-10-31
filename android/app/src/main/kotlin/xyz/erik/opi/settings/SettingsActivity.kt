// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.os.Bundle
import android.view.Gravity
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.CompositionLocalProvider
import xyz.erik.opi.ime.LocalSceneTheme
import xyz.erik.opi.jni.EngineLoader

/**
 * 设置页宿主（A5）：launcher 入口（manifest 直接声明本类为 launcher，无 MainActivity）。
 * 启动即编排 luna 资产加载（幂等：size 校验重拷；失败回退内置词库）——
 * 与 IME 侧共享 Rust 静态单例。`EngineLoader` 保证 luna 每进程只 install 一次：
 * 重复 install 会重置 IME 正在用的输入方案/模式（用户报障「打开设置后输入无反应」）。
 *
 * 2026-10-28：装载搬 [EngineLoader.io] 后台线程（首次 36MB 拷贝/校验 + Rust install
 * 此前同步占着 launcher 主线程）。设置页首帧就读引擎状态，所以**装完才组合**，
 * 装载期只给一行占位。
 */
class SettingsActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(
            TextView(this).apply {
                text = "正在加载词库…"
                gravity = Gravity.CENTER
            },
        )
        EngineLoader.io.execute {
            // 装载返回值 = 小欧表情的降级信号（false = 回退内置 35 词库 → 折断天线）。
            // 不能丢：设置页那只小欧就是给用户看引擎状态的，丢了就永远显示「一切正常」。
            val dictionaryLoaded = EngineLoader.load(this)
            runOnUiThread {
                if (!isFinishing && !isDestroyed) {
                    val scene = ImePreferences.from(this).scene
                    setContent {
                        CompositionLocalProvider(LocalSceneTheme provides scene) {
                            SettingsScreen(dictionaryDegraded = !dictionaryLoaded)
                        }
                    }
                }
            }
        }
    }
}
