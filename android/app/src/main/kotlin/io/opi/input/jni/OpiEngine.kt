package io.opi.input.jni

import io.opi.input.engine.OpiEngineApi

/**
 * Rust 引擎 JNI 入口（A1：crates/opi-ffi/src/jni.rs 注册表）。
 *
 * JNI_OnLoad RegisterNatives 注册 18 个方法，类名 io/opi/input/jni/OpiEngine，
 * 方法名与签名必须与 Rust 侧注册表逐一吻合（签名断裂 → UnsatisfiedLinkError）。
 * so 文件名 libopi_ffi.so（cargokit libname=opi_ffi）。
 *
 * 实现 OpiEngineApi 以便 EngineController 在 JVM 测试中注入假引擎。
 */
object OpiEngine : OpiEngineApi {
    init {
        System.loadLibrary("opi_ffi")
    }

    /** load(path: String?) -> Boolean。null/空串 → 内置回退词库；坏路径 → false。 */
    external fun load(path: String?): Boolean

    /** inputKey(ch: String) -> String。单字符外（空/多字符/非 ASCII）返回空串。 */
    override external fun inputKey(ch: String): String

    override external fun backspace()

    override external fun clear()

    /** select(index: Int) -> String。越界返回空串（旧语义）。 */
    override external fun select(index: Int): String

    /** switchMode(mode: Int)。0=Pinyin 1=English 2=Number 3=Symbol，越界忽略。 */
    override external fun switchMode(mode: Int)

    override external fun setShift(on: Boolean)

    /** inputSpace() -> String。英文模式提交 buffer。 */
    override external fun inputSpace(): String

    /** candidates(limit: Int) -> String[]。仅文本数组；JNI 可能返回 null。 */
    override external fun candidates(limit: Int): Array<String>?

    override external fun buffer(): String

    override external fun mode(): Int

    /** searchSymbols(keyword: String) -> String[]。JNI 可能返回 null。 */
    external fun searchSymbols(keyword: String): Array<String>?

    /** symbolBlocks() -> String。JSON：`[{id,start,end,name,common}]`。 */
    external fun symbolBlocks(): String

    /** symbolsInBlock(id: Short) -> String[]。JNI 可能返回 null。 */
    external fun symbolsInBlock(id: Short): Array<String>?

    external fun learnerEnabled(): Boolean

    external fun setLearner(enabled: Boolean)

    external fun clearUserWords()

    external fun exportUserWords(): String
}
