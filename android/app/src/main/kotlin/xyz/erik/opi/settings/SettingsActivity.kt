// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import xyz.erik.opi.jni.EngineLoader

/**
 * 设置页宿主（A5）：launcher 入口（manifest 直接声明本类为 launcher，无 MainActivity）。
 * 启动即编排 luna 资产加载（幂等：size 校验重拷；失败回退内置词库）——
 * 与 IME 侧共享 Rust 静态单例。`EngineLoader` 保证 luna 每进程只 install 一次：
 * 重复 install 会重置 IME 正在用的输入方案/模式（用户报障「打开设置后输入无反应」）。
 */
class SettingsActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // 装载返回值 = 小欧表情的降级信号（false = 回退内置 35 词库 → 折断天线）。
        // 不能丢：设置页那只小欧就是给用户看引擎状态的，丢了就永远显示「一切正常」。
        val dictionaryLoaded = EngineLoader.load(this)
        setContent { SettingsScreen(dictionaryDegraded = !dictionaryLoaded) }
    }
}
