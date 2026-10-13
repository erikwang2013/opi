// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 模式循环的**纯函数**门禁（2026-10-14）：`opi-ctl mode` 的应答怎么翻译成新模式
//! token（[cycleModeToken]）。界面拿它决定按钮上新画的档位，所以纪律钉在这里：
//! **认不得的应答一律空串**（绝不能当模式名画上去）——最要紧的指纹是 `ok`：那是
//! 插件把 mode 接到了旧路径（Trigger 的通用成功应答），放过去条上就会显示 "ok"。
//!
//! 下半天追加（同 2026-10-14）：state 行 `mode=` 格的读侧（[parseImeState] 的
//! mode 那一格）也钉在本文件 —— 与应答面**同一个 token 表**（[MODE_TOKENS]），
//! 只是方向相反；缺键/认不得 = `""` = 未知，**解析层不回落**「拼音」（回落归消费方：
//! 状态栏那侧，见 SkinAndStatus.kt 的 barMode）—— 「不在这里回落」是刻意的，
//! 见 [ImeState.mode] 段。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Test

class ModeCycleTest {

    @Test
    fun `each token passes through verbatim`() {
        assertEquals("pinyin", cycleModeToken(0, "pinyin"))
        assertEquals("en26", cycleModeToken(0, "en26"))
    }

    @Test
    fun `wubi is accepted ahead of the engine`() {
        // 引擎现在不会发 wubi（五笔未落地）：先收下是前向兼容 —— 加档时桌面不必
        // 跟着发版。这条钉的是「放行是有意的」，不是「现在真会拿到」（同 CLI 侧）。
        assertEquals("wubi", cycleModeToken(0, "wubi"))
    }

    @Test
    fun `ok reply is not a mode`() {
        // 最要紧的负向：`ok` = mode 走错了路（旧插件/旧路径）——必须是空串，
        // 否则「中英」按钮旁边会多出一个写着 ok 的档位。
        assertEquals("", cycleModeToken(0, "ok"))
    }

    @Test
    fun `broken payloads map to empty`() {
        // 取值域是**枚举**，不是「非空串」：大小写/带空白/带尾巴/别的词都不认。
        // （首行的空白已由 runCapture 修掉；这里不依赖它 —— 两道都得挡。）
        for (bad in listOf(null, "", "zh", "PINYIN", "En26", "en26 ", "en26:pinyin", "pinyin/五笔")) {
            assertEquals("$bad 不该被认成模式 token", "", cycleModeToken(0, bad))
        }
    }

    @Test
    fun `nonzero exit is a failure no matter the payload`() {
        // 退出码语义见 opi-ctl 头部那张表：3=通道不通、5=应答异形……都回空串
        // （调用方按「什么都没切成」处理，显示保持原样）。带 token 的载荷也不放行：
        // 非 0 就是没切成。契约的失败面是空串（没有给用户看的原因）。
        for (code in listOf(2, 3, 4, 5, 1)) {
            assertEquals("rc=$code", "", cycleModeToken(code, "en26"))
        }
    }

    // ---------- state 的 `mode=` 格（2026-10-14 追加：状态栏初始档的真源） ----------
    // 与上面同一条 token 取值域，只是方向相反（应答 → 档位 / 状态行 → 档位）。

    @Test
    fun `state mode cell fills the tier`() {
        assertEquals(
            "pinyin",
            parseImeState("lang=zh;trad=off;punct=on;fw=on;im=opi;mode=pinyin").mode,
        )
        assertEquals(
            "en26",
            parseImeState("lang=en;trad=off;punct=on;fw=off;im=;mode=en26").mode,
        )
        assertEquals(
            "wubi",
            parseImeState("lang=zh;trad=off;punct=on;fw=on;im=;mode=wubi").mode,
        )
        // 面外态（繁体）的**服务端形态**：mode 格报 pinyin（读法仍是拼音），简繁由
        // trad 格自述 —— 2026-10-14 改判后 `trad=on` 与 `mode=pinyin` 同现是正常的
        // （初版报 en26 = 中文输入画英文档位的假标签，见 cpp/opi_ctl_mode.h 的改判记录）。
        val tradLine = parseImeState("lang=zh;trad=on;punct=on;fw=on;im=;mode=pinyin")
        assertEquals("繁体行 mode 格 = pinyin（面外态收成读法）", "pinyin", tradLine.mode)
        assertEquals("简繁由 trad 格表达，与 mode 各管各", true, tradLine.trad)
    }

    @Test
    fun `missing or unknown mode reads as unknown`() {
        // 老插件/老 CLI 的输出：整行**没有** mode 格 —— 契约明写这不是解析失败。
        // 读出来是**空串 = 未知**，解析层**不**回落「拼音」：回落是消费方（状态栏
        // barMode）的事；代它回落的话，它那道「老版本无 mode 位 ⇒ 拼音」的门禁
        // 就算删掉回落代码也照样绿 —— 那条门禁就没了分辨力。
        assertEquals(
            "",
            parseImeState("lang=zh;trad=off;punct=on;fw=on;im=opi").mode,
        )
        // 认不得的值与缺键走同一条路（同 im 的口径：只有那一格落缺省，不判坏整行）；
        // **不原样收** —— mode 有枚举面（im 没有），域外的串（首推 "ok"）不能交给
        // 界面画档位。
        for (bad in listOf("", "quanpin", "PINYIN", "En26", "ok", "五笔", "wubi2")) {
            assertEquals(
                "mode=$bad 该读成未知",
                "",
                parseImeState("lang=zh;trad=off;fw=on;im=;mode=$bad").mode,
            )
        }
        // 整行读不到（通道不通/压根没跑成）：全落缺省，mode 同为未知 —— 状态栏照旧
        // 用自己那个初始档（那是它的回落，不在这边代劳）。
        // JUnit4 的 message 重载是 (message, expected, actual) —— 消息必须打头。
        assertEquals("整行读不到 ⇒ 未知", "", parseImeState(null).mode)
    }

    @Test
    fun `state line without mode keeps every other cell`() {
        // 加字段**不顶掉既有行**：老输出（五格）解析出来的其余格与从前逐个一致
        // （Kotlin 的 readImeState 是活体消费方，这条钉住「加键是纯追加」）。
        val st = parseImeState("lang=en;trad=on;punct=off;fw=off;im=keyboard-us")
        assertEquals(
            ImeState(
                zh = false,
                trad = true,
                punct = false,
                fullwidth = false,
                im = "keyboard-us",
            ),
            st,
        )
        assertEquals("缺 mode 格 ⇒ 未知", "", st.mode)
    }
}
