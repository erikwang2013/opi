// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

// ⇧ 状态机（2026-11-08 从 EngineController 拆出守 500 行门禁）。
// 扩展函数与成员函数同包调用，UI/测试不用改 import。

/**
 * ⇧ **单击 = 锁定大写；再点一次取消**（2026-10-15 用户裁决「切换到英文，让英文字母
 * 大写的按钮，点击了，就要锁定大写，再次点击取消大写」——对 2026-10-10「点 = 单次、
 * 长按 = 锁定」那条的改判）。
 *
 * 三态里「单次大写」（SINGLE）没被砍掉，挪到 [shiftLongPress]（换手势，不丢能力）——
 * `consumeSingleShift` 与字母键面的大写显示都按状态走，两处不用改。
 */
fun EngineController.shiftTap() {
    if (shiftState == ShiftState.LOCK) {
        shiftState = ShiftState.OFF
        api.setShift(false)
    } else {
        shiftState = ShiftState.LOCK
        api.setShift(true)
    }
    // 2026-10-27：引擎 set_shift 清联想 ⇒ 必须重读候选（原先只动 UI 三态）。
    refresh()
}

/** ⇧ 长按 = **单次大写**（SINGLE）：下一个字母大写后由 [consumeSingleShift] 复位。 */
fun EngineController.shiftLongPress() {
    shiftState = ShiftState.SINGLE
    api.setShift(true)
    refresh()
}

/** single 态消费后复位（lock 不受影响）。 */
fun EngineController.consumeSingleShift() {
    if (shiftState == ShiftState.SINGLE) {
        shiftState = ShiftState.OFF
        api.setShift(false)
    }
}
