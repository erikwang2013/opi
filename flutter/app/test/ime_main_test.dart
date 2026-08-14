import 'package:app/candidates/candidate_bar.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/ime/ime_main.dart';
import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fake_ime_channel.dart';

void main() {
  setUpAll(() async {
    await RustLib.init();
  });

  testWidgets('全流程：输入 → 候选 → 点击提交经通道', (tester) async {
    final ctrl = (await tester.runAsync(() async {
      return await EngineController.load();
    }))!;
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));

    await tester.tap(find.text('w'));
    await tester.pump();
    await tester.tap(find.text('o'));
    await tester.pump();
    expect(ctrl.buffer, 'wo');

    await tester.tap(find.text('我'));
    await tester.pump();
    expect(channel.commits, ['我']);
    expect(ctrl.buffer, '');

    // 空 buffer 空格直传
    await tester.tap(find.text('空格'));
    await tester.pump();
    expect(channel.commits, ['我', ' ']);
    await tester.runAsync(() async => ctrl.dispose());
  });

  testWidgets('英文模式候选栏退化为 EN 模式条，字母直传', (tester) async {
    final ctrl = (await tester.runAsync(() async {
      return await EngineController.load();
    }))!;
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));

    // modeLabel 动态显示：拼音模式 '中'（M5 起不再用 🌐）
    await tester.tap(find.text('中'));
    await tester.pump();
    // EN 模式条反馈（切换有明确区域变化），无候选词
    expect(find.text('EN'), findsOneWidget);
    expect(find.text('字母直接上屏'), findsOneWidget);
    await tester.tap(find.text('a'));
    await tester.pump();
    expect(channel.commits, ['a']);
    expect(ctrl.buffer, '');

    // 切回拼音显示 '英'
    await tester.tap(find.text('英'));
    await tester.pump();
    expect(ctrl.mode, ApiMode.pinyin);
    expect(find.text('EN'), findsNothing);
    await tester.runAsync(() async => ctrl.dispose());
  });

  testWidgets('窄屏（360dp）键盘不溢出', (tester) async {
    tester.view.physicalSize = const Size(1080, 2400);
    tester.view.devicePixelRatio = 3.0;
    addTearDown(tester.view.reset);
    final ctrl = (await tester.runAsync(() async {
      return await EngineController.load();
    }))!;
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));
    await tester.runAsync(() async => ctrl.dispose());
  });
}
