import 'package:app/candidates/candidate_bar.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('显示缓冲与候选，点击回调索引', (tester) async {
    // 计划注记纠偏：RustLib.init() 幂等可在 testWidgets 内完成，但 EngineController.load()
    // 的 FFI 异步结果经 isolate 端口消息送达，在 FakeAsync 中永不派发（实测挂起至 10 分钟
    // 超时），必须用 tester.runAsync() 包住 FFI 等待。
    late EngineController ctrl;
    await tester.runAsync(() async {
      await RustLib.init();
      ctrl = await EngineController.load();
      ctrl.input('w');
      ctrl.input('o');
    });
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
    ctrl.dispose();
  });
}
