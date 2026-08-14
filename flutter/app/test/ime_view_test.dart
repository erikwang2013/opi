import 'package:app/candidates/candidate_bar.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/ime/ime_main.dart';
import 'package:app/keyboards/number_pad.dart';
import 'package:app/keyboards/qwerty.dart';
import 'package:app/keyboards/symbol_panel.dart';
import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fake_ime_channel.dart';

void main() {
  setUpAll(() async {
    await RustLib.init();
  });

  testWidgets('123 短按 → number，数字直传，ABC 回 qwerty 且 pending 已提交', (tester) async {
    final ctrl = (await tester.runAsync(() async => await EngineController.load()))!;
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));

    await tester.tap(find.text('w'));
    await tester.pump();
    expect(ctrl.buffer, 'w');
    final first = ctrl.candidates.first.text; // 打开面板前的首个候选（'我'）

    await tester.tap(find.text('123'));
    await tester.pump();
    expect(find.byType(NumberPad), findsOneWidget);
    expect(find.byType(QwertyKeyboard), findsNothing);
    expect(channel.commits, [first]); // 打开面板前提交 pending 拼音

    await tester.tap(find.text('5'));
    await tester.pump();
    expect(channel.commits, [first, '5']);
    expect(ctrl.mode, ApiMode.pinyin); // 面板开关不触碰引擎模式

    await tester.tap(find.text('ABC'));
    await tester.pump();
    expect(find.byType(QwertyKeyboard), findsOneWidget);
    expect(ctrl.buffer, ''); // pending 已提交，无残留
    await tester.runAsync(() async => ctrl.dispose());
  });

  testWidgets('123 长按不再开 symbol（HIGH-1）；123 短按 → number；?123 → symbol', (tester) async {
    final ctrl = (await tester.runAsync(() async => await EngineController.load()))!;
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));

    // 回归：长按 123 曾吞 tap 打开 symbol，现在只会释放为 tap → number
    await tester.longPress(find.text('123'));
    await tester.pump();
    expect(find.byType(SymbolPanel), findsNothing);
    expect(find.byType(NumberPad), findsOneWidget);

    await tester.tap(find.text('?123'));
    await tester.pump();
    expect(find.byType(SymbolPanel), findsOneWidget);

    final first = ctrl.symbols.common.first;
    await tester.tap(find.text(first.text));
    await tester.pump();
    expect(channel.commits, [first.text]);
    expect(find.byType(SymbolPanel), findsOneWidget); // 面板不关闭
    expect(ctrl.mode, ApiMode.pinyin);

    // symbol → qwerty（ABC），再 123 短按 → number，?123 → symbol
    await tester.tap(find.text('ABC'));
    await tester.pump();
    expect(find.byType(QwertyKeyboard), findsOneWidget);
    await tester.tap(find.text('123'));
    await tester.pump();
    expect(find.byType(NumberPad), findsOneWidget);
    await tester.tap(find.text('?123'));
    await tester.pump();
    expect(find.byType(SymbolPanel), findsOneWidget);
    await tester.runAsync(() async => ctrl.dispose());
  });

  testWidgets('editorChanged（输入目标切换）取消组合串、面板回 qwerty', (tester) async {
    final ctrl = (await tester.runAsync(() async => await EngineController.load()))!;
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));

    // 挂载即注册 handler
    expect(channel.editorChangedHandler, isNotNull);

    await tester.tap(find.text('w'));
    await tester.pump();
    expect(ctrl.buffer, 'w');

    // 面板开着时收到通知 → 回 qwerty
    await tester.tap(find.text('123'));
    await tester.pump();
    expect(find.byType(NumberPad), findsOneWidget);
    channel.editorChangedHandler?.call();
    await tester.pump();
    expect(find.byType(QwertyKeyboard), findsOneWidget);
    expect(ctrl.buffer, '');

    // 组合串未提交时收到通知 → 取消而非泄入编辑器
    await tester.tap(find.text('w'));
    await tester.pump();
    expect(ctrl.buffer, 'w');
    channel.editorChangedHandler?.call();
    await tester.pump();
    expect(ctrl.buffer, '');
    expect(channel.commits.length, 1); // 仅面板打开那次提交，无新泄入
    await tester.runAsync(() async => ctrl.dispose());
  });

  testWidgets('number/symbol 视图不显示候选栏，回 qwerty 无残留', (tester) async {
    final ctrl = (await tester.runAsync(() async => await EngineController.load()))!;
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));

    await tester.tap(find.text('w'));
    await tester.pump();
    expect(find.byType(CandidateBar), findsOneWidget);

    final first = ctrl.candidates.first.text; // 打开面板前的首候选（luna 排序）
    await tester.tap(find.text('123'));
    await tester.pump();
    expect(find.byType(CandidateBar), findsNothing);
    expect(find.byType(NumberPad), findsOneWidget);
    expect(channel.commits, [first]); // pending 已随面板打开提交

    await tester.tap(find.text('ABC'));
    await tester.pump();
    expect(find.byType(CandidateBar), findsNothing); // pending 已提交，无候选可显
    expect(ctrl.buffer, '');

    await tester.tap(find.text('123'));
    await tester.pump();
    await tester.tap(find.text('?123'));
    await tester.pump();
    expect(find.byType(CandidateBar), findsNothing);
    expect(find.byType(SymbolPanel), findsOneWidget);
    expect(ctrl.buffer, ''); // 无候选不提交
    await tester.runAsync(() async => ctrl.dispose());
  });
}
