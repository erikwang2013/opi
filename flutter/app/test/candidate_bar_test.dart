import 'package:app/candidates/candidate_bar.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('显示缓冲与候选，点击回调索引', (tester) async {
    // runAsync<T> 返回 Future<T?>，此处用 ! 断言非空。
    final ctrl = (await tester.runAsync(() async {
      await RustLib.init();
      final c = await EngineController.load();
      c.input('w');
      c.input('o');
      return c;
    }))!;
    final tapped = <int>[];
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: CandidateBar(controller: ctrl, onTap: tapped.add),
      ),
    ));

    expect(find.text('wo'), findsOneWidget); // 缓冲显示在候选栏（无 composing）
    expect(find.text('我'), findsOneWidget); // 候选 top-8 首项
    await tester.tap(find.text('我'));
    expect(tapped, [0]);
    await tester.runAsync(() async => ctrl.dispose());
  });
}
