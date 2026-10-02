// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * [petMood] 纯函数单测。
 *
 * 宠物的表情是引擎状态的一面镜子 —— 判错就会在真机上撒谎：明明一个候选都没有
 * 却笑嘻嘻，或者用户关了学习它还当自己在记事。这里把五种情绪的判定钉死。
 *
 * 末尾三条专门钉「引擎降级」与「用户打错」不是一回事。
 */
class OpiPetMoodTest {

    @Test
    fun emptyBufferIsIdle() {
        assertEquals(PetMood.IDLE, petMood(buffer = "", candidateCount = 0, learnerEnabled = true))
    }

    @Test
    fun candidatesMakeItWait() {
        assertEquals(
            PetMood.WAITING,
            petMood(buffer = "ni", candidateCount = 8, learnerEnabled = true),
        )
    }

    @Test
    fun bufferWithoutCandidatesIsPuzzled() {
        // 乱码拼音（如 "zzz"）无候选：不能装作 IDLE，它得表达「我也没辙」。
        assertEquals(
            PetMood.PUZZLED,
            petMood(buffer = "zzz", candidateCount = 0, learnerEnabled = true),
        )
    }

    @Test
    fun learnerOffOutranksEverything() {
        // 学习关闭优先于缓冲与候选：睡着了就不记事，表情不能反映输入状态。
        assertEquals(PetMood.SLEEPY, petMood(buffer = "", candidateCount = 0, learnerEnabled = false))
        assertEquals(PetMood.SLEEPY, petMood(buffer = "ni", candidateCount = 8, learnerEnabled = false))
    }

    @Test
    fun degradedOutranksSleepy() {
        // 词库没装上时学习自然也没在记事，但 SLEEPY（自己关掉了学习）与
        // DEGRADED（引擎坏了）不是一回事：坏消息优先，别让用户以为自己关过。
        assertEquals(
            PetMood.DEGRADED,
            petMood(buffer = "", candidateCount = 0, learnerEnabled = false, dictionaryDegraded = true),
        )
    }

    @Test
    fun degradedOutranksNonEmptyBuffer() {
        // 引擎降级与「用户打没打字、有没有候选」无关：有候选也得折断天线，
        // 不能拿 WAITING 或 PUZZLED 把引擎自己的故障盖过去。
        assertEquals(
            PetMood.DEGRADED,
            petMood(buffer = "ni", candidateCount = 8, learnerEnabled = true, dictionaryDegraded = true),
        )
        assertEquals(
            PetMood.DEGRADED,
            petMood(buffer = "zzz", candidateCount = 0, learnerEnabled = true, dictionaryDegraded = true),
        )
    }

    @Test
    fun notDegradedKeepsOldJudgements() {
        // 默认参数必须等价于「引擎正常」：宿主还没接上真信号时不能改变旧判定。
        assertEquals(
            PetMood.WAITING,
            petMood(buffer = "ni", candidateCount = 8, learnerEnabled = true, dictionaryDegraded = false),
        )
    }
}
