// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

//! 引擎协议枚举（2026-11-02 自 EngineController.kt 机械搬出：状态快照改动后
//! 那个文件越 500 行门禁）。同包顶层类型，引用方一字未改。

/** 引擎模式（JNI mode() 返回值：0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional）。 */
enum class EngineMode(val value: Int) {
    PINYIN(0), ENGLISH(1), NUMBER(2), SYMBOL(3), TRADITIONAL(4);

    companion object {
        fun fromInt(v: Int) = entries.firstOrNull { it.value == v } ?: PINYIN
    }
}

/** ⇧ 状态：off / single（下个字母大写后自动复位）/ lock（持续大写）。 */
enum class ShiftState { OFF, SINGLE, LOCK }

/**
 * 输入方案（JNI `scheme()` 返回值）：0=全键盘 1=拼音9键 2=拼音14键 3=笔画 4=双拼 5=五笔。
 *
 * 「输入模式」= 方案 × 模式的组合（拼音9键 = NINE + PINYIN）；繁体模式是**模式**轴上的
 * 事（[EngineMode.TRADITIONAL]），与方案正交 —— 9 键也能切繁体。
 * 双拼的**布局**（自然码/微软/…）在 [OpiEngineApi.shuangpinLayout] 上，与本枚举正交。
 * ⚠️ 整数表是**协议**：opi-ffi 的 `scheme_from_int`、`OpiEngine.kt` 的注释、
 * 这里三处逐值对应（jni_contract 只保签名不保语义）。
 */
enum class InputScheme(val value: Int) {
    FULL(0), NINE(1), FOURTEEN(2), STROKE(3), SHUANGPIN(4), WUBI(5);

    companion object {
        fun fromInt(v: Int) = entries.firstOrNull { it.value == v } ?: FULL
    }
}
