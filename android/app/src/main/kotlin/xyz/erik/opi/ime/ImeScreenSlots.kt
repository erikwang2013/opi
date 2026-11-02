// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.remember
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.composingText
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.petMood

/**
 * ImeScreen 根组合只订阅这几项**粗粒度**结果（2026-11-02 性能项 3）：
 * 每键变化的缓冲/候选数在 [rememberImeScreenSlots] 里归并为可见性与情绪档，
 * 只有它们真正翻转时才让整屏重组；候选栏/拼音行各自读 controller 的细粒度状态。
 *
 * [englishMode] 必须直读 `controller.mode`：繁→英切换时 [englishIdle] 可能保持
 * false，只有 mode 变化才能把 CandidateBar 的 `inEnglish` 刷新过来。
 */
internal data class ImeScreenSlots(
    val englishMode: Boolean,
    val englishIdle: Boolean,
    val composingRow: Boolean,
    val candidatesShowable: Boolean,
    val toolbarMood: PetMood,
)

/** [ImeScreenSlots] 的派生状态：底层状态变、结果不变时不会使读取方失效。 */
@Composable
internal fun rememberImeScreenSlots(
    controller: EngineController,
    state: ImeState,
): State<ImeScreenSlots> = remember(controller, state) {
    derivedStateOf {
        ImeScreenSlots(
            englishMode = controller.mode == EngineMode.ENGLISH,
            englishIdle = controller.mode == EngineMode.ENGLISH &&
                controller.buffer.isEmpty() && controller.candidates.isEmpty(),
            composingRow = composingRowActive(
                controller.composingText(),
                controller.wubiHint,
                controller.buffer,
                controller.candidates.size,
            ),
            candidatesShowable = state.candidatesShowable,
            toolbarMood = petMood(
                buffer = controller.buffer,
                candidateCount = controller.candidates.size,
                learnerEnabled = controller.learnerEnabled,
                dictionaryDegraded = controller.dictionaryDegraded,
            ),
        )
    }
}
