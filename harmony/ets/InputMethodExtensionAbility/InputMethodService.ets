// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**。
// 连 InputMethodExtensionAbility 这个基类**能不能这样继承**都没有验证过 ——
// 见 README「最不确定的 API」第 1 条。
//
// 输入法扩展的入口。**这个类故意只做转发**，一行输入法逻辑都不放 ——
// 与官方示例（InputMethodService 只管 IME Kit，逻辑在 KeyboardController）
// 同构。逻辑全在 KeyboardController，引擎逻辑全在 crates/engine-core。

import { InputMethodExtensionAbility } from '@kit.IMEKit';
import { Want } from '@kit.AbilityKit';
import { KeyboardController } from './KeyboardController';

export default class InputMethodService extends InputMethodExtensionAbility {
  /**
   * 扩展被拉起时调用。
   *
   * ⚠️ 这个方法可能被**反复调用**（输入法进程会被系统回收后再拉起），
   * 所以里面必须幂等 —— KeyboardController.init() 自身有 started 挡重入。
   */
  onCreate(want: Want): void {
    KeyboardController.getInstance().init(this.context);
  }

  /**
   * 扩展销毁时调用。
   * **必须真的释放**：面板是系统资源，不 destroy 会泄漏；
   * 监听不 off 会在下次 onCreate 时叠加重复回调（表现：一次按键上屏两次）。
   */
  onDestroy(): void {
    KeyboardController.getInstance().release();
  }
}
