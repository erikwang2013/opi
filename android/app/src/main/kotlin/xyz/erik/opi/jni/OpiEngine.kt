// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.jni

import xyz.erik.opi.engine.OpiEngineApi
import xyz.erik.opi.keyboard.SymbolApi

/**
 * Rust 引擎 JNI 入口（A1：crates/opi-ffi/src/jni.rs 注册表）。
 *
 * JNI_OnLoad RegisterNatives 注册本文件声明的全部方法（**条数以
 * `crates/opi-ffi/tests/jni_contract.rs` 那道门禁为准，这里不写数字**），
 * 类名 xyz/erik/opi/jni/OpiEngine，方法名与签名必须与 Rust 侧注册表逐一吻合。
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

    /**
     * toggleFullwidth() -> Boolean：全角 ↔ 半角，返回**切换后的新状态**（状态栏直接
     * 拿去刷新）。未装载 → false。
     */
    external fun toggleFullwidth(): Boolean

    /**
     * fullwidthState() -> Boolean：全角开关读侧。**每一次 `switchMode` 之后都必须重读**
     * —— 切模式把它重置为该模式的默认值（拼音/繁体全角，英文/数字/符号半角）。
     *
     * ⚠️ 本出口与 `chinesePunct` **不是一档**：那条硬规则只对全角成立。
     */
    external fun fullwidthState(): Boolean

    /**
     * toggleSymbol() -> String：符号面板开关，**返回需要上屏的文本（空串 = 无提交）**。
     *
     * ⚠️ 返回的**不是**「刚切出来的那个符号」，而是切进面板前那截缓冲的待上屏文本。
     * **拿不到插入通道的调用方不要调它**。调用后须重读 mode / buffer / candidates /
     * fullwidth 四样（内部走了一次 `switchMode`，它会重置全角）。
     */
    external fun toggleSymbol(): String

    /**
     * chinesePunct() -> Boolean：中文标点表开关读侧。未装载 → false。
     *
     * ⚠️ **与 `fullwidthState` 不是一档**：本档是用户偏好，`switchMode` **不重置它**
     * ——「每次 `switchMode` 后必须重读全角」那条硬规则**不适用于本出口**。
     * 关掉本档**不等于半角**：表未命中的字符仍由全角那一档管。
     */
    external fun chinesePunct(): Boolean

    /**
     * setChinesePunct(on: Boolean)：设置中文标点表是否生效。**无返回值** ——
     * 它是设置项（勾选框知道要设成什么值）；要拿结果请读 `chinesePunct()`。
     * **写入口有两个**：本方法是设置项，触发键走 `toggleChinesePunct()`。
     */
    external fun setChinesePunct(on: Boolean)

    /**
     * toggleChinesePunct() -> Boolean：中文标点表开关的**触发键入口**，返回**切换后的
     * 新状态**（状态栏 / 勾选框直接拿去刷新）。与 `setChinesePunct` 是同一档的两个写入口。
     *
     * ⚠️ **重读规则与 `toggleFullwidth` 不通用**：本档是用户偏好，`switchMode` 与
     * `toggleSymbol` **都不重置它**（同 `chinesePunct` 那一段）。未装载 → false。
     */
    external fun toggleChinesePunct(): Boolean

    /**
     * fuzzyEnabled()/jianpinEnabled() -> Boolean：「属性设置」四个旋钮中的两个读侧
     * （写侧见下面的 setFuzzy/setJianpin）。⚠️ 未装载 → false 是**哨兵**，不是
     * "开关默认关" —— 默认值是开的（引擎侧 `RankOptions::default()`）。
     */
    override external fun fuzzyEnabled(): Boolean
    override external fun jianpinEnabled(): Boolean

    /** setFuzzy(on: Boolean)：模糊音开关（设置项，无返回值；要结果读 fuzzyEnabled()）。 */
    override external fun setFuzzy(on: Boolean)

    /** setJianpin(on: Boolean)：简拼开关（同上）。 */
    override external fun setJianpin(on: Boolean)

    /**
     * scheme() -> Int：输入方案读侧（0=全键盘 1=拼音9键 2=拼音14键 3=笔画）。
     * ⚠️ 未装载 → 0 是**哨兵**（同 [fuzzyEnabled] 一档），不是"用户选的就是全键盘"。
     */
    override external fun scheme(): Int

    /**
     * setScheme(scheme: Int)：输入方案写侧（移动端「输入模式」，2026-10-09）。
     * **无返回值**（设置项）；越界忽略。⚠️ **换方案清缓冲** —— 调用方重读
     * buffer/候选/页码（见 EngineController.switchScheme）。
     */
    override external fun setScheme(scheme: Int)

    /** inputSpace() -> String。英文模式提交 buffer。 */
    override external fun inputSpace(): String

    /** candidates(limit: Int) -> String[]。仅文本数组；JNI 可能返回 null。 */
    override external fun candidates(limit: Int): Array<String>?

    override external fun buffer(): String

    override external fun mode(): Int

    /** searchSymbols(keyword: String) -> String[]。JNI 可能返回 null。 */
    override external fun searchSymbols(keyword: String): Array<String>?

    /** emojiSymbols() -> String[]。引擎按 emoji 属性判定的条目；JNI 可能返回 null。 */
    override external fun emojiSymbols(): Array<String>?

    /** symbolBlocks() -> String。JSON：`[{id,start,end,name,common}]`。 */
    override external fun symbolBlocks(): String

    /** symbolsInBlock(id: Short) -> String[]。JNI 可能返回 null。 */
    override external fun symbolsInBlock(id: Short): Array<String>?

    /** loadTrad(path: String) -> Boolean。坏路径/引擎未加载 → false（繁体模式回退简体库）。 */
    external fun loadTrad(path: String): Boolean

    /** loadStroke(path: String) -> Boolean。笔画词典（stroke.opid）。坏路径/未装载 → false（笔画方案回退普通路径）。 */
    external fun loadStroke(path: String): Boolean

    override external fun learnerEnabled(): Boolean

    override external fun setLearner(enabled: Boolean)

    external fun clearUserWords()

    external fun exportUserWords(): String

    /** 导入用户词 JSON。返回导入条数；负数表示格式非法（既有状态不变）。 */
    external fun importUserWords(json: String): Int

    /** 删除一个用户词（连同其频次）。 */
    override external fun removeUserWord(text: String)
}
