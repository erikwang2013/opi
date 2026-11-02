// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

/**
 * 偏好恢复：多个开关一次性写入后**只 refresh 一次**。逐个 setter 会让 onReady
 * 连做多次全量回读（含 candidates(512)），冷启动白白多花 5–20ms。
 * 从 EngineController.kt 拆出守 500 行门禁。
 */
internal fun EngineController.applySwitches(
        learn: Boolean,
        fuzzy: Boolean,
        fuzzyPairs: Int,
        useFuzzyPairs: Boolean,
        jianpin: Boolean,
        correction: Boolean,
        shuangpinLayout: Int,
    ) {
        api.setLearner(learn)
        if (useFuzzyPairs) api.setFuzzyPairs(fuzzyPairs) else api.setFuzzy(fuzzy)
        api.setJianpin(jianpin)
        api.setCorrection(correction)
        api.setShuangpinLayout(shuangpinLayout)
        refresh()
    }

