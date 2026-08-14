import 'package:app/candidates/candidate_bar.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  setUpAll(() async {
    await RustLib.init();
  });

  List<ApiCandidate> fakeCands(int n) => [
        for (var i = 0; i < n; i++)
          ApiCandidate(text: 'c$i', kind: ApiCandidateKind.hanzi, score: BigInt.zero),
      ];

  test('64 候选切片 8/屏 + 翻页钳制', () async {
    final ctrl = await EngineController.load();
    ctrl.candidates = fakeCands(20);
    expect(ctrl.candidatePageCount, 3);
    expect(ctrl.pageCandidates.length, 8);
    expect(ctrl.pageCandidates.first.text, 'c0');

    ctrl.nextPage();
    expect(ctrl.candidatePage, 1);
    expect(ctrl.pageCandidates.first.text, 'c8');
    ctrl.nextPage();
    expect(ctrl.pageCandidates.length, 4); // 末页 4 项
    ctrl.nextPage(); // 越界钳制
    expect(ctrl.candidatePage, 2);

    ctrl.prevPage();
    expect(ctrl.candidatePage, 1);
    ctrl.prevPage();
    ctrl.prevPage(); // 越界钳制
    expect(ctrl.candidatePage, 0);
    ctrl.dispose();
  });

  test('buffer 变化重置页，shift 不重置', () async {
    final ctrl = await EngineController.load();
    ctrl.input('w');
    ctrl.input('o');
    expect(ctrl.candidatePage, 0);

    ctrl.candidates = fakeCands(20);
    ctrl.nextPage();
    expect(ctrl.candidatePage, 1);
    ctrl.input('n'); // buffer 变化 → 重置
    expect(ctrl.candidatePage, 0);

    ctrl.candidates = fakeCands(20);
    ctrl.nextPage();
    expect(ctrl.candidatePage, 1);
    ctrl.shiftTap(); // shift 不重置页
    expect(ctrl.candidatePage, 1);
    ctrl.dispose();
  });

  test('selectFromPage 屏内下标 → 绝对下标', () async {
    final ctrl = await EngineController.load();
    ctrl.input('t');
    ctrl.input('a');
    expect(ctrl.candidates.length, greaterThan(1)); // luna 词库多候选
    final second = ctrl.candidates[1].text;
    expect(ctrl.selectFromPage(1), second); // 屏内 1 = 绝对 1
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });

  testWidgets('候选栏翻页指示器与屏内点击', (tester) async {
    final ctrl = (await tester.runAsync(() async => await EngineController.load()))!;
    ctrl.candidates = fakeCands(20);
    final tapped = <int>[];
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: CandidateBar(controller: ctrl, onTap: tapped.add),
      ),
    ));

    expect(find.text('1/3'), findsOneWidget);
    await tester.tap(find.byIcon(Icons.chevron_right));
    await tester.pump();
    expect(find.text('2/3'), findsOneWidget);
    expect(find.text('c8'), findsOneWidget);
    await tester.tap(find.text('c8'));
    expect(tapped, [0]); // 屏内下标 0 = 绝对下标 8
    await tester.runAsync(() async => ctrl.dispose());
  });
}
