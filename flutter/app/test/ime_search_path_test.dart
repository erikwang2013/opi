import 'package:app/engine/engine_controller.dart';
import 'package:app/ime/ime_main.dart';
import 'package:app/keyboards/qwerty.dart';
import 'package:app/keyboards/symbol_panel.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fake_ime_channel.dart';

/// 补缺：coder 标注的搜索态集成缺口 —— 符号面板搜索框聚焦后，
/// qwerty 字母盘输入拼音 → 防抖 → 网格出结果 → 点选提交。
void main() {
  setUpAll(() async {
    await RustLib.init();
  });

  testWidgets('符号面板搜索态：聚焦 → qwerty 搜索盘 → 结果 → 提交', (tester) async {
    final ctrl = (await tester.runAsync(() async => await EngineController.load()))!;
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));

    // 进入符号面板：123 → 数字面板 → ?123 → 符号面板（123 长按已移除）
    await tester.tap(find.text('123'));
    await tester.pump();
    await tester.tap(find.text('?123'));
    await tester.pump();
    expect(find.byType(SymbolPanel), findsOneWidget);

    // 点击搜索框获得焦点 → 联动 qwerty 字母盘（IME 内无系统键盘）
    await tester.tap(find.byType(TextField));
    await tester.pump();
    expect(find.byType(QwertyKeyboard), findsOneWidget);

    // 字母键进入搜索框（qwerty 程序化路径，非 TextField 键入）
    await tester.tap(find.text('h'));
    await tester.tap(find.text('e'));
    await tester.pump(const Duration(milliseconds: 300)); // 250ms 防抖
    await tester.pump();

    // 焦点态下搜索结果网格应可见（'he' → ♥）
    expect(find.byType(SymbolPanel), findsOneWidget,
        reason: '搜索态应保留面板（含结果网格），而非被 qwerty 整体替换');
    expect(find.text('♥'), findsWidgets, reason: '拼音 he 应命中 ♥');

    // 点选提交
    await tester.tap(find.text('♥').first);
    await tester.pump();
    expect(channel.commits, ['♥']);

    await tester.runAsync(() async => ctrl.dispose());
  });
}
