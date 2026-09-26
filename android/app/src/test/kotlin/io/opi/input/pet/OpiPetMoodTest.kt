package io.opi.input.pet

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * [petMood] 纯函数单测。
 *
 * 宠物的表情是引擎状态的一面镜子 —— 判错就会在真机上撒谎：明明一个候选都没有
 * 却笑嘻嘻，或者用户关了学习它还当自己在记事。这里把四种情绪的判定钉死。
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
}
