import 'package:app/engine/engine_controller.dart';
import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  // 与 engine_flow_test.dart 相同：Api.loadFallback 需先 RustLib.init()（幂等）。
  setUpAll(() async {
    await RustLib.init();
  });

  test('controller drives buffer/mode/candidates', () async {
    final ctrl = await EngineController.load();

    ctrl.input('w');
    ctrl.input('o');
    expect(ctrl.buffer, 'wo');
    expect(ctrl.candidates.length, greaterThan(0));
    expect(ctrl.candidates.first.text, '我');

    final committed = ctrl.select(0);
    expect(committed, '我');
    expect(ctrl.buffer, '');

    ctrl.backspace();
    expect(ctrl.buffer, '');

    // 空格键：拼音模式提交首候选，缓冲清空（engine_flow_test 已验证）。
    ctrl.input('n');
    ctrl.input('i');
    ctrl.inputSpace();
    expect(ctrl.buffer, '');

    ctrl.switchMode(ApiMode.english);
    expect(ctrl.mode, ApiMode.english);

    ctrl.dispose();
  });

  test('inputSpace returns committed text', () async {
    final ctrl = await EngineController.load();
    ctrl.input('w');
    ctrl.input('o');
    expect(ctrl.inputSpace(), '我'); // 当前返回 void，编译失败
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });

  test('controller notifies listeners', () async {
    final ctrl = await EngineController.load();
    var notified = 0;
    ctrl.addListener(() => notified++);
    ctrl.input('w');
    expect(notified, greaterThan(0));
    ctrl.dispose();
  });
}
