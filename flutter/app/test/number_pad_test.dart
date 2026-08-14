import 'package:app/keyboards/number_pad.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('数字/标点键直传 + 功能键回调', (tester) async {
    final keys = <String>[];
    var symbol = 0;
    var letters = 0;
    var space = 0;
    var backspace = 0;
    var enter = 0;
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: NumberPad(
          onKey: keys.add,
          onSymbol: () => symbol++,
          onLetters: () => letters++,
          onSpace: () => space++,
          onBackspace: () => backspace++,
          onEnter: () => enter++,
        ),
      ),
    ));

    for (final k in ['1', '2', '3', '4', '5', '6', '7', '8', '9', ',', '0', '.']) {
      await tester.tap(find.text(k));
    }
    expect(keys, ['1', '2', '3', '4', '5', '6', '7', '8', '9', ',', '0', '.']);

    await tester.tap(find.text('?123'));
    await tester.tap(find.text('ABC'));
    await tester.tap(find.text('空格'));
    await tester.tap(find.text('⌫'));
    await tester.tap(find.text('↵'));
    expect(symbol, 1);
    expect(letters, 1);
    expect(space, 1);
    expect(backspace, 1);
    expect(enter, 1);
  });
}
