// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import xyz.erik.opi.jni.OpiEngine

/**
 * 清除用户词库的**两半**：落盘缓存（[UserWordStore.clearPersisted]）+ 引擎内存
 * （`OpiEngine.clearUserWords`）。app 设置页与键盘内设置面板共用这一处，避免两条 UI
 * 各自拼顺序。
 *
 * 顺序不能反（见 `clearPersisted` 的三步）：先作废在途快照、取消两实例的防抖保存并
 * 把删除排进共享 io 队列，再清引擎内存 —— 反了的话 IME 侧排着的保存会在清词后把旧词
 * 重新写回文件，重启时「已清除」又被撤销。
 *
 * 引擎清空做成参数：本函数可纯 JVM 测（测试传真实现），生产默认用真引擎。
 */
fun clearUserWords(
    store: UserWordStore,
    clearEngine: () -> Unit = { OpiEngine.clearUserWords() },
) {
    store.clearPersisted()
    clearEngine()
}
