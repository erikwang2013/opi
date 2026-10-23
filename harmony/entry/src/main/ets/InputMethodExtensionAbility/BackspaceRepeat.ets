// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 长按 ⌫ 的连续删除计时器（从 KeyboardController 拆出：那个文件已在上限边上，
// 与 StatePublish 同一条先例）。模块级单实例：整个输入法只有一个 ⌫ 在按。
//
// 为什么不用 `LongPressGesture({ repeat: true })`：它的重复间隔**就是** duration
// （默认 500ms，官方 ts-basic-gestures-longpressgesture.md 的示例注释明写），
// 调小 duration 又会把长按阈值一起调小 ⇒ 普通短按会被判成长按。故 500ms 长按
// 成立后由本计时器以固定 60ms 重复，松手/取消由手势回调停止。

/** 重复间隔：60ms ≈ 16 格/秒，接近桌面键盘按住退格的体感。 */
const REPEAT_MS: number = 60;
/** 单次按住的上限（≈18s）：手势结束回调万一丢失，也不至于无限删宿主正文。 */
const MAX_TICKS: number = 300;

let timer: number = -1;
let ticks: number = 0;

/** 启动（幂等）：立即执行一次，之后每 REPEAT_MS 执行一次；已在跑则忽略。 */
export function start(tick: () => void): void {
  if (timer !== -1) {
    return;
  }
  ticks = 0;
  tick();
  timer = setInterval(() => {
    tick();
    if (++ticks >= MAX_TICKS) {
      stop();
    }
  }, REPEAT_MS);
}

/** 停止（幂等）：松手 / 手势取消 / 输入会话结束都可重复调用。 */
export function stop(): void {
  if (timer === -1) {
    return;
  }
  clearInterval(timer);
  timer = -1;
}
