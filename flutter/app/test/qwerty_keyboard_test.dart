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
    await tester.tap(find.text('🌐'));
    expect(space, 1);
    expect(backspace, 1);
    expect(enter, 1);
    expect(modeSwitch, 1);
  });
}
