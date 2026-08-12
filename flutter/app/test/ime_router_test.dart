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

  Future<(EngineController, ImeRouter, FakeImeChannel)> setup() async {
    final ctrl = await EngineController.load();
    final channel = FakeImeChannel();
    return (ctrl, ImeRouter(ctrl, channel), channel);
  }

  test('空 buffer：空格/退格/回车直传通道', () async {
    final (ctrl, router, channel) = await setup();
    router.handleSpace();
    router.handleBackspace();
    router.handleEnter();
    expect(channel.commits, [' ']);
    expect(channel.deleteCount, 1);
    expect(channel.enterCount, 1);
    ctrl.dispose();
  });

  test('拼音：字母进缓冲，空格提交首候选', () async {
    final (ctrl, router, channel) = await setup();
    router.handleKey('w');
    router.handleKey('o');
    expect(ctrl.buffer, 'wo');
    router.handleSpace();
    expect(channel.commits, ['我']);
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });

  test('拼音：退格缩短缓冲，回车提交首候选', () async {
    final (ctrl, router, channel) = await setup();
    router.handleKey('w');
    router.handleKey('o');
    router.handleBackspace();
    expect(ctrl.buffer, 'w');
    router.handleKey('o');
    router.handleEnter();
    expect(channel.commits, ['我']);
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });

  test('英文模式：空缓冲字母直传', () async {
    final (ctrl, router, channel) = await setup();
    ctrl.switchMode(ApiMode.english);
    router.handleKey('a');
    expect(channel.commits, ['a']);
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });

  test('候选选择提交所选文本', () async {
    final (ctrl, router, channel) = await setup();
    router.handleKey('w');
    router.handleKey('o');
    router.handleCandidate(0);
    expect(channel.commits, ['我']);
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });
}
