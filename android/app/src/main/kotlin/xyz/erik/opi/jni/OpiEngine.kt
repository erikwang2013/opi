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

    /** pageSize() -> Int。每页候选数（引擎 `router::PAGE_SIZE`，别写死 8）。 */
    override external fun pageSize(): Int

    /** switchMode(mode: Int)。0=Pinyin 1=English 2=Number 3=Symbol，越界忽略。 */
    override external fun switchMode(mode: Int)

    override external fun setShift(on: Boolean)

    /**
     * toggleFullwidth() -> Boolean：全角 ↔ 半角，返回**切换后的新状态**（状态栏直接
     * 拿去刷新）。未装载 → false。
     */
    override external fun toggleFullwidth(): Boolean

    /**
     * fullwidthState() -> Boolean：全角开关读侧。**每一次 `switchMode` 之后都必须重读**
     * —— 切模式把它重置为该模式的默认值（拼音/繁体全角，英文/数字/符号半角）。
     *
     * ⚠️ 本出口与 `chinesePunct` **不是一档**：那条硬规则只对全角成立。
     */
    override external fun fullwidthState(): Boolean

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

    /**
     * fuzzyPairs(): Int / setFuzzyPairs(mask: Int)：模糊音**逐对**启用集
     * （2026-10-13 用户裁决「可多选类似 z=zh」）。位序 = 引擎 `FUZZY_PAIRS` 数组序：
     * bit0 zh/z、bit1 ch/c、bit2 sh/s、bit3 an/ang、bit4 en/eng、bit5 in/ing、
     * bit6 n/l、bit7 f/h；`0xFF` = 全开（= setFuzzy(true)）。
     * ⚠️ 未装载 → 读侧 0 是**哨兵**（同 [fuzzyEnabled] 一档）；越界位由 C 层钳掉。
     */
    override external fun fuzzyPairs(): Int
    override external fun setFuzzyPairs(mask: Int)

    /** correctionEnabled()/setCorrection(on)：拼音纠错开关（2026-10-13）。
     *  ⚠️ 未装载 → 读侧 false 是**哨兵**；真缺省是 true（对齐主流输入法）。 */
    override external fun correctionEnabled(): Boolean
    override external fun setCorrection(on: Boolean)

    /** setJianpin(on: Boolean)：简拼开关（同上）。 */
    override external fun setJianpin(on: Boolean)

    /**
     * scheme() -> Int：输入方案读侧（0=全键盘 1=拼音9键 2=拼音14键 3=笔画 4=双拼 5=五笔；
     * 真源 `engine-core/src/scheme.rs` 的 `Scheme::code` —— 2026-10-14 补上漏写的 5）。
     * ⚠️ 未装载 → 0 是**哨兵**（同 [fuzzyEnabled] 一档），不是"用户选的就是全键盘"。
     */
    override external fun scheme(): Int

    /**
     * setScheme(scheme: Int)：输入方案写侧（移动端「输入模式」，2026-10-09）。
     * **无返回值**（设置项）；越界忽略。⚠️ **换方案清缓冲** —— 调用方重读
     * buffer/候选/页码（见 EngineController.switchScheme）。
     */
    override external fun setScheme(scheme: Int)

    /**
     * shuangpinLayout() -> Int / setShuangpinLayout(index: Int)：双拼布局
     * （2026-10-13；[scheme] 取 4 时的配套设置）。0=自然码 1=微软 2=小鹤
     * 3=智能ABC 4=紫光 5=拼音加加（顺序 = 引擎 `shuangpin_tables.rs` 的表序，
     * 设置页的名字表有锁步门禁钉着）。
     * ⚠️ 未装载 → 读侧 0 是**哨兵**；负数钳到 0、过大钳到末位。
     * **不清缓冲**（区别于 [setScheme]）—— 缓冲里的键序按新布局重新解码。
     */
    override external fun shuangpinLayout(): Int
    override external fun setShuangpinLayout(index: Int)

    /**
     * loadWubi(path: String) -> Boolean：五笔码表（wubi86.opid / wubi98.opid，2026-10-14，M4）。
     * 空/坏路径/未装载 → false（方案回退普通路径，与 `loadStroke` 同姿态）。
     * ⚠️ 装载**不换方案**：要用五笔还要 `setScheme(5)`（[InputScheme.SHUANGPIN] 隔壁那位）。
     */
    external fun loadWubi(path: String): Boolean

    /**
     * 五笔旋钮族（2026-10-14，M4）。整数表（与 C ABI 同值，四端协议）：
     * · [wubiStyle] 0=纯五笔 1=五笔拼音混输（越界忽略；未装载 → 0 是哨兵）
     * · [wubiMixedEntry] 0=直接字母 1=z键（只在混输下有语义；越界忽略）
     * · [wubiReturnStyle] 0=清空 1=上屏原文（设置位；**消费判据见 [enterAction]**）
     * · [wubiCandidateLimit] 1..=9（**越界钳到区间**，与双拼布局同纪律）；缺省 5
     * · [wubiAutoCommit] 四码唯一自动上屏（缺省 true；未装载 → false 是哨兵）
     * 全部**不清缓冲**（与 [setScheme] 相反：它们只改「同一缓冲怎么装配候选」）。
     */
    external fun wubiStyle(): Int
    external fun setWubiStyle(style: Int)
    external fun wubiMixedEntry(): Int
    external fun setWubiMixedEntry(entry: Int)
    external fun wubiCandidateLimit(): Int
    external fun setWubiCandidateLimit(n: Int)
    external fun wubiReturnStyle(): Int
    external fun setWubiReturnStyle(style: Int)

    /**
     * enterAction() -> Int：**回车（缓冲非空时）该做什么**（2026-10-14 接线）——
     * 0=选首候选、1=清空缓冲、2=上屏原文（随后读 [buffer] 取原文）。
     * 判据在引擎（五笔读「回车行为」设置位；非五笔 / 空缓冲恒 0 = 各端既有行为）。
     */
    override external fun enterAction(): Int
    external fun wubiAutoCommit(): Boolean
    external fun setWubiAutoCommit(on: Boolean)

    /**
     * wubiNextKeys() -> String：编码提示（当前缓冲可接续的键，拼成串；非五笔态 → 空串）。
     * ⚠️ 引擎侧每次调用全量重算（O(26×前缀查询)）—— **别在每帧渲染里调**，
     * 缓冲变化时取一次缓存住。
     */
    override external fun wubiNextKeys(): String

    /**
     * switchLanguage(mode: Int, scheme: Int)：**语言切换**（中↔英/中↔繁）—— 换模式与
     * 方案但**保留缓冲**（2026-10-09 用户裁决「中文输入的时候切换英文，拼音保留可继续
     * 使用」）。取值同 [switchMode]/[setScheme]；任一越界整体忽略。调用方仍要 refresh。
     */
    override external fun switchLanguage(mode: Int, scheme: Int)

    /** inputSpace() -> String。英文模式提交 buffer。 */
    override external fun inputSpace(): String

    /** candidates(limit: Int) -> String[]。仅文本数组；JNI 可能返回 null。 */
    override external fun candidates(limit: Int): Array<String>?

    /**
     * candidateReadings() -> String[]：候选拼音读数（已分词），**按读音去重保序**，
     * 与最近一次 [candidates] 同源同一次排名 —— 9 键左列用（2026-10-09 用户裁决
     * 「一列多个、可滑动查看」；2026-10-12 用户裁决「左列去重」）。
     * 非 9/14 键方案、或缓冲已变（提交/退格）⇒ 空数组。调用顺序：先 [candidates]。
     */
    override external fun candidateReadings(): Array<String>?

    /**
     * pinReading(reading: String)：**钉读音**（空串 = 解除）—— 9 键左列点一行拼音后
     * **不确认**，候选收窄到**读音恰等**的条目（2026-10-09 用户裁决「不要直接确认」；
     * 2026-10-16 裁决收紧为恰等）。缓冲一变自动作废。调用方重读 candidates/读数。
     */
    override external fun pinReading(reading: String)

    override external fun buffer(): String

    /** bufferIsCarried() -> Boolean：缓冲是否由别的模式带过来（语言切换保留缓冲时打标）。 */
    override external fun bufferIsCarried(): Boolean

    /**
     * displayBuffer() -> String：**显示用**的拼音读数（9 键方案下是带分词的读音
     * `ni'hao`；其余方案与 [buffer] 相同）。
     *
     * ⚠️ 与 [buffer] **是两个出口，别混用**：`buffer()` 是缓冲本体，宿主拿它做
     * **行为判定**（回退/空格/回车的「缓冲非空？」）—— 2026-10-09 把读数塞进
     * buffer() 出口，9 键无候选时读数空 ⇒ 回退删了编辑器文字、引擎缓冲没退，
     * 连着打字看不出变化（用户报「键盘没反应」）。
     */
    override external fun displayBuffer(): String

    override external fun pendingText(): String

    override external fun mode(): Int

    /** searchSymbols(keyword: String) -> String[]。JNI 可能返回 null。 */
    override external fun searchSymbols(keyword: String): Array<String>?

    /** emojiSymbols() -> String[]。引擎按 emoji 属性判定的条目；JNI 可能返回 null。 */
    override external fun emojiSymbols(): Array<String>?

    /** symbolBlocks() -> String。JSON：`[{id,start,end,name,common}]`。 */
    override external fun symbolBlocks(): String

    /**
     * emojiGroups() -> String。JSON：`[{count,name,start}]`（**serde 字典序**，别按
     * name,start,count 写顺序正则；解析照 `symbolBlocks` 逐字段抓）—— 分类条
     * （2026-10-12 用户裁决「中间是表情分类」）。`start` 对齐 `emojiSymbols()` 的下标：
     * 点分类 = 取 `[start, start+count)` 切片。**纯静态表**（与引擎装载无关），未装载也有值。
     */
    override external fun emojiGroups(): String?

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

    /** 导入自定义短语 JSON。返回导入条数；负数表示格式非法（既有短语表不变）。 */
    external fun setCustomPhrases(json: String): Int

    /** 导出自定义短语 JSON（与 [setCustomPhrases] 同一 schema/顺序）。 */
    external fun exportCustomPhrases(): String

    external fun setCustomPhrasesEnabled(on: Boolean)

    external fun customPhrasesEnabled(): Boolean

    /** 英文自定义短语（2026-10-24）：与中文套同 schema，表与开关完全独立。 */
    external fun setEnglishCustomPhrases(json: String): Int

    external fun exportEnglishCustomPhrases(): String

    external fun setEnglishCustomPhrasesEnabled(on: Boolean)

    external fun englishCustomPhrasesEnabled(): Boolean

    /** 英文短语是否实际可命中（开关开 + 表非空）：普通英文据此改走缓冲。 */
    override external fun englishPhrasesActive(): Boolean

    /** 删除一个用户词（连同其频次）。 */
    override external fun removeUserWord(text: String)

    // ---------- 导入词库（2026-10-14，M3）----------
    // 契约四端同一份（完整裁决见 opi-ffi 的 api/dict.rs 头注）：
    // · **名字 = 文件名词干**（`foo.tsv` → `foo`），与 `DisabledDicts` 名单同口径；
    // · **数据留在 FFI 层内存里**：停用后再启用**不依赖原文件还在**；
    // · 系统词典（luna/trad/stroke）不接受 [setDictEnabled] —— 按 `dictList` 的
    //   `system: true` 隐藏开关；
    // · [importDict] 收**绝对路径**（Android 侧是 `filesDir/dicts/foo.tsv`）。

    /**
     * importDict(path: String) -> Int。读一个 UTF-8 的 tsv 词库（三列
     * `词<TAB>拼音<TAB>频次`，**词在前**）当一个导入层，返回条目数。
     * 失败（读不到 / 格式全坏 / 没有有效条目 / 未装载）→ **-1**，既有导入层不动。
     * 同名重导入 = **覆盖**并置为启用。
     */
    external fun importDict(path: String): Int

    /**
     * dictList() -> String。JSON 数组：系统词典在前（luna/trad/stroke，`system: true`），
     * 导入层随后（导入顺序）；每项 `{name, count, enabled, system}`。
     * **未装载 → `[]`**。⚠️ 键序是 serde 字典序（count/enabled/name/system）——按**键**取。
     */
    external fun dictList(): String?

    /**
     * setDictEnabled(name: String, on: Boolean) -> Boolean。装载 / 卸载一条**导入**词库
     * （原子）。已是目标态 → true（幂等）；名字认不得（含系统词典）/ 未装载 → false。
     */
    external fun setDictEnabled(name: String, on: Boolean): Boolean

    /**
     * removeDict(name: String) -> Boolean。把一条**导入**词库从内存里删掉；
     * **文件是宿主的事**（本层不碰磁盘）。名字认不得 / 已经不在 / 未装载 → false。
     */
    external fun removeDict(name: String): Boolean

    // ---------- 局域网跨端剪贴板（opi-clip，2026-11-01）----------
    // 注册表在 `crates/opi-ffi/src/jni_clip.rs`，且**注册在 OpiEngine 这个类上**
    // （JNI_OnLoad 的 find_class 写死），所以声明只能放在这里 —— 新开一个 object
    // 调不到。13 条必须与注册表逐条同名同签名；句柄**不可跨线程**，统一由
    // `xyz.erik.opi.clip.ClipBridge` 的专用线程持有。
    // 失败哨兵：create → 0、poll/peers/status → null、其余 → false / -1；
    // 原因读 clipLastError()（按线程记录）。

    /** create(configJson) -> 句柄；失败 → 0。只解析配置，不启动。 */
    external fun clipCreate(configJson: String?): Long

    /** destroy(handle)：停止并释放；0 是空句柄，安全。 */
    external fun clipDestroy(handle: Long)

    /** start(handle) -> Boolean：绑定端口并起内核线程；幂等。 */
    external fun clipStart(handle: Long): Boolean

    /** stop(handle)：幂等。 */
    external fun clipStop(handle: Long)

    /** localText(handle, text) -> Int：0=已发布 1=防回环抑制 -1=失败。 */
    external fun clipLocalText(handle: Long, text: String?): Int

    /** pollEvent(handle, timeoutMs) -> JSON；null=失败，空串=无事件。 */
    external fun clipPollEvent(handle: Long, timeoutMs: Int): String?

    /** pairStart(handle, peerId)：peerId 是 32 位十六进制实例 ID。 */
    external fun clipPairStart(handle: Long, peerId: String): Boolean

    /** pairConfirm(handle, peerId, accept)：收到 pairing_prompt 后调用一次。 */
    external fun clipPairConfirm(handle: Long, peerId: String, accept: Boolean): Boolean

    /** pairCancel(handle, peerId)：等价于 pairConfirm(accept=false)。 */
    external fun clipPairCancel(handle: Long, peerId: String): Boolean

    /** peers(handle) -> JSON 数组；失败 → null，未启动 → `[]`。 */
    external fun clipPeers(handle: Long): String?

    /** forgetPeer(handle, peerId)：移除信任；不存在也算成功。 */
    external fun clipForgetPeer(handle: Long, peerId: String): Boolean

    /** status(handle) -> JSON；失败 → null。 */
    external fun clipStatus(handle: Long): String?

    /** lastError() -> String：空串 = 没有错误（按线程记录）。 */
    external fun clipLastError(): String
}
