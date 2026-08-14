import 'package:app/keyboards/qwerty.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('字母键与功能键回调', (tester) async {
    final keys = <String>[];
    var space = 0;
    var backspace = 0;
    var enter = 0;
    var modeSwitch = 0;
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: QwertyKeyboard(
          onKey: keys.add,
          onSpace: () => space++,
          onBackspace: () => backspace++,
          onEnter: () => enter++,
          onModeSwitch: () => modeSwitch++,
        ),
      ),
    ));

    await tester.tap(find.text('a'));
    await tester.tap(find.text('z'));
    expect(keys, ['a', 'z']);

    await tester.tap(find.text('空格'));
    await tester.tap(find.text('⌫'));
    await tester.tap(find.text('↵'));
    // modeLabel 默认 '中'（M5：🌐 图标已改为动态 modeLabel）
    await tester.tap(find.text('中'));
    expect(space, 1);
    expect(backspace, 1);
    expect(enter, 1);
    expect(modeSwitch, 1);
  });

  testWidgets('⇧ 与 123 短按/长按回调', (tester) async {
    var shift = 0;
    var shiftLong = 0;
    var number = 0;
    var symbolLong = 0;
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: QwertyKeyboard(
          onKey: (_) {},
          onSpace: () {},
          onBackspace: () {},
          onEnter: () {},
          onModeSwitch: () {},
          onShift: () => shift++,
          onShiftLongPress: () => shiftLong++,
          onNumber: () => number++,
          onSymbolLongPress: () => symbolLong++,
        ),
      ),
    ));

    await tester.tap(find.text('⇧'));
    await tester.longPress(find.text('⇧'));
    await tester.tap(find.text('123'));
    await tester.longPress(find.text('123'));
    expect(shift, 1);
    expect(shiftLong, 1);
    // HIGH-1 修复：123 无长按，长按释放为 tap → onNumber；onSymbolLongPress 不再触发
    expect(number, 2);
    expect(symbolLong, 0);
  });
}
