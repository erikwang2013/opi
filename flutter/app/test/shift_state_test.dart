import 'package:app/engine/engine_controller.dart';
import 'package:app/ime/ime_router.dart';
import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fake_ime_channel.dart';

void main() {
  setUpAll(() async {
    await RustLib.init();
  });

  test('off→single→off→lock→off 状态转移', () async {
    final ctrl = await EngineController.load();
    expect(ctrl.shiftState, ShiftState.off);
    ctrl.shiftTap();
    expect(ctrl.shiftState, ShiftState.single);
    ctrl.shiftTap();
    expect(ctrl.shiftState, ShiftState.off);
    ctrl.shiftTap();
    ctrl.shiftLongPress();
    expect(ctrl.shiftState, ShiftState.lock);
    ctrl.shiftTap(); // lock 短按解除
    expect(ctrl.shiftState, ShiftState.off);
    ctrl.dispose();
  });

  test('consumeSingleShift 仅 single 态复位', () async {
    final ctrl = await EngineController.load();
    ctrl.shiftLongPress();
    ctrl.consumeSingleShift();
    expect(ctrl.shiftState, ShiftState.lock); // lock 不受影响
    ctrl.shiftTap();
    ctrl.consumeSingleShift();
    expect(ctrl.shiftState, ShiftState.off); // single → off
    ctrl.dispose();
  });

  test('English 空 buffer：single 大写直传并复位', () async {
    final ctrl = await EngineController.load();
    final channel = FakeImeChannel();
    final router = ImeRouter(ctrl, channel);
    ctrl.switchMode(ApiMode.english);
    ctrl.shiftTap();
    router.handleKey('a');
    expect(channel.commits, ['A']);
    expect(ctrl.shiftState, ShiftState.off);
    router.handleKey('b');
    expect(channel.commits, ['A', 'b']); // 复位后小写
    ctrl.dispose();
  });

  test('English 空 buffer：lock 持续大写', () async {
    final ctrl = await EngineController.load();
    final channel = FakeImeChannel();
    final router = ImeRouter(ctrl, channel);
    ctrl.switchMode(ApiMode.english);
    ctrl.shiftLongPress();
    router.handleKey('a');
    router.handleKey('b');
    expect(channel.commits, ['A', 'B']);
    expect(ctrl.shiftState, ShiftState.lock);
    ctrl.dispose();
  });

  test('English 非空 buffer：走引擎，set_shift 生效', () async {
    final ctrl = await EngineController.load();
    final channel = FakeImeChannel();
    final router = ImeRouter(ctrl, channel);
    ctrl.switchMode(ApiMode.english);
    // router 直传路径只处理空 buffer；非空 buffer 需直接经引擎进入
    ctrl.input('a');
    expect(ctrl.buffer, 'a');
    ctrl.shiftTap();
    router.handleKey('b'); // buffer 非空 → 引擎 input，set_shift 生效
    expect(ctrl.buffer, 'aB');
    ctrl.dispose();
  });

  test('Pinyin 模式 shift 无效果', () async {
    final ctrl = await EngineController.load();
    final channel = FakeImeChannel();
    final router = ImeRouter(ctrl, channel);
    ctrl.shiftTap();
    router.handleKey('w');
    expect(ctrl.buffer, 'w'); // composer 转小写（事实 7）
    expect(channel.commits, isEmpty);
    ctrl.dispose();
  });
}
