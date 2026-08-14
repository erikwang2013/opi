package io.opi.input.settings

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import io.opi.input.jni.EngineLoader

/**
 * 设置页宿主（A5）：launcher 入口（manifest 声明 .MainActivity 继承本类）。
 * 启动即编排 luna 资产加载（幂等：size 校验重拷；失败回退内置词库）——
 * 与 IME 侧共享 Rust 静态单例，先到先载，后到跳过。
 */
class SettingsActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        EngineLoader.load(this)
        setContent { SettingsScreen() }
    }
}
