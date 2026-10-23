// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.content.Context
import xyz.erik.opi.jni.OpiEngine
import java.io.File
import java.io.FileOutputStream

/**
 * 自定义短语的**宿主真源**（2026-10-23）：启用开关 + 整张短语表落在
 * `filesDir/opi_custom_phrases.json`（[FILE_NAME]），进程重启后由宿主装回引擎。
 *
 * 为什么真源在宿主而不是引擎：引擎侧 `setCustomPhrases` 收整表 JSON、内存态、不落盘；
 * 输入法与设置页共享同一个 Rust 单例，但进程会被系统杀。宿主不存 = 用户配的短语每次
 * 重启凭空消失。
 *
 * 纯 Kotlin（`java.io.File` + 注入的引擎口），JVM 测试直接构造；[from] 只是 Android
 * 调用的组装糖（同 [ImePreferences.from] 的做法）。
 *
 * 失败语义分两层：**写盘失败**由 [save] 交回调用方（此时 UI 不该变）；**推引擎失败**
 * 由 [push] 交回（宿主文件已经是对的，IME 下次启动会重推，设置页只提示）。
 */
class CustomPhraseStore(
    private val file: File,
    /** OpiEngine.setCustomPhrases：返回条数，负数 = 引擎拒收（整表不动）。 */
    private val pushPhrases: (String) -> Int,
    /** OpiEngine.setCustomPhrasesEnabled。 */
    private val pushEnabled: (Boolean) -> Unit,
    /** OpiEngine.customPhrasesEnabled：写口无返回值，只能读回确认启用态真被接受。 */
    private val readEnabled: () -> Boolean,
) {
    /** 启用态缺省**开**：与主流输入法一致；没有短语时开关本来也不影响输入。 */
    data class State(val enabled: Boolean = true, val phrases: List<CustomPhrase> = emptyList())

    companion object {
        /** filesDir 下的短语文件名（与 luna.opid / 用户词同目录）。 */
        const val FILE_NAME = "opi_custom_phrases.json"

        fun from(context: Context): CustomPhraseStore = CustomPhraseStore(
            file = File(context.filesDir, FILE_NAME),
            pushPhrases = { OpiEngine.setCustomPhrases(it) },
            pushEnabled = { OpiEngine.setCustomPhrasesEnabled(it) },
            readEnabled = { OpiEngine.customPhrasesEnabled() },
        )
    }

    /**
     * 读宿主文件。缺失/读失败/内容非法 → **默认值（启用、空表）**，不删文件：
     * 坏文件留着可人工排查，下一次保存会原子覆盖它（同 UserWordStore.load 的姿态）。
     */
    fun load(): State {
        val text = try {
            file.readText()
        } catch (e: Exception) {
            return State()
        }
        return when (val d = CustomPhraseCodec.decode(text)) {
            is CustomPhraseDecode.Ok -> State(d.enabled ?: true, d.phrases)
            is CustomPhraseDecode.Invalid -> State()
        }
    }

    /**
     * 原子写（同目录唯一 tmp → 写 + fsync → rename）。返回 null = 成功；否则是
     * **面向用户**的原因。写失败时旧文件原样保留。
     */
    fun save(state: State): String? {
        val dir = file.parentFile ?: return "没有可写的目录"
        val tmp = try {
            File.createTempFile(file.name + ".", ".tmp", dir)
        } catch (e: Throwable) {
            return "建临时文件失败：${e.message}"
        }
        return try {
            FileOutputStream(tmp).use { out ->
                out.write(
                    CustomPhraseCodec.encodeState(state.enabled, state.phrases).toByteArray(Charsets.UTF_8),
                )
                out.fd.sync()
            }
            if (tmp.renameTo(file)) {
                null
            } else {
                tmp.delete()
                "重命名失败"
            }
        } catch (e: Throwable) {
            tmp.delete()
            "写盘失败：${e.message}"
        }
    }

    /**
     * 整表推给引擎。返回 null = 接受；否则是**面向用户**的原因。
     *
     * 短语与启用态都推：引擎侧是两个独立写口，且启用态写口无返回值 —— 用读回比对
     * （[readEnabled]）才能知道它有没有真的收下；未装载的引擎读回哨兵值会不等。
     */
    fun push(state: State): String? {
        val json = CustomPhraseCodec.encodeCanonical(state.phrases)
        val count = try {
            pushPhrases(json)
        } catch (e: Throwable) {
            // 死 .so 抛的是 Error 而非 Exception（见 UserWordStore.load 的同一条注释）
            return "引擎不可用（${e::class.simpleName}）"
        }
        if (count < 0) return "引擎拒绝了短语表"
        return try {
            pushEnabled(state.enabled)
            if (readEnabled() != state.enabled) "引擎未接受启用状态" else null
        } catch (e: Throwable) {
            "引擎不可用（${e::class.simpleName}）"
        }
    }

    /**
     * 装载后整表重推（IME 每次建输入视图时调用一次）。
     * 失败**不抛**（不能因此崩输入法），交给 [onError] 留痕。
     */
    fun loadAndPush(onError: (String) -> Unit = {}): State {
        val state = load()
        push(state)?.let(onError)
        return state
    }
}
