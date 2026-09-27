// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.jni

import io.opi.input.engine.OpiEngineApi
import io.opi.input.keyboard.SymbolApi

/**
 * Rust 引擎 JNI 入口（A1：crates/opi-ffi/src/jni.rs 注册表）。
 *
 * JNI_OnLoad RegisterNatives 注册 21 个方法，类名 io/opi/input/jni/OpiEngine，
 * 方法名与签名必须与 Rust 侧注册表逐一吻合。
 * ⚠️ 少一条**不是**「新方法调不到」，而是**整个 so 装不上**（`RegisterNatives` 整表
 * 失败 → `JNI_OnLoad` 返 0），报错文本却是误导性的 `unsupported JNI version 0x00000000`。
 * 所以这里是**三份声明面**之一：改 `jni.rs` 注册表必须同改本文件、`android/jni_smoke/Main.java`
 * （门禁：`crates/opi-ffi/tests/jni_contract.rs`）。
 * so 文件名 libopi_ffi.so（cargokit libname=opi_ffi）。
 *
 * 实现 OpiEngineApi/SymbolApi 以便 EngineController/SymbolCatalog 在 JVM 测试中注入假实现。
 */
object OpiEngine : OpiEngineApi, SymbolApi {
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
    override external fun searchSymbols(keyword: String): Array<String>?

    /** symbolBlocks() -> String。JSON：`[{id,start,end,name,common}]`。 */
    override external fun symbolBlocks(): String

    /** symbolsInBlock(id: Short) -> String[]。JNI 可能返回 null。 */
    override external fun symbolsInBlock(id: Short): Array<String>?

    /** loadTrad(path: String) -> Boolean。坏路径/引擎未加载 → false（繁体模式回退简体库）。 */
    external fun loadTrad(path: String): Boolean

    external fun learnerEnabled(): Boolean

    external fun setLearner(enabled: Boolean)

    external fun clearUserWords()

    external fun exportUserWords(): String

    /** 导入用户词 JSON。返回导入条数；负数表示格式非法（既有状态不变）。 */
    external fun importUserWords(json: String): Int

    /** 删除一个用户词（连同其频次）。 */
    override external fun removeUserWord(text: String)
}
