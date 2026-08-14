import 'package:app/engine/engine_controller.dart';
import 'package:app/settings/settings_page.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  setUpAll(() async {
    await RustLib.init();
  });

  Future<EngineController> loadCtrl() async => EngineController.load();

  testWidgets('学习开关切换调用 setLearner', (tester) async {
    final ctrl = (await tester.runAsync(() async => await loadCtrl()))!;
    expect(ctrl.learnerEnabled, isTrue); // M1 默认开
    await tester.pumpWidget(MaterialApp(home: SettingsPage(controller: ctrl)));

    await tester.tap(find.byType(Switch));
    await tester.pump();
    expect(ctrl.learnerEnabled, isFalse);
    await tester.tap(find.byType(Switch));
    await tester.pump();
    expect(ctrl.learnerEnabled, isTrue);
    await tester.runAsync(() async => ctrl.dispose());
  });

  testWidgets('清除用户词库：确认流', (tester) async {
    final ctrl = (await tester.runAsync(() async => await loadCtrl()))!;
    // 先制造一个用户词，验证清除生效
    ctrl.input('w');
    ctrl.input('o');
    final selected = ctrl.candidates.first.text; // luna 首位（不硬编码）
    ctrl.select(0);
    expect(ctrl.exportUserWords().contains(selected), isTrue);

    await tester.pumpWidget(MaterialApp(home: SettingsPage(controller: ctrl)));
    await tester.tap(find.text('清除用户词库'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('清除'));
    await tester.pumpAndSettle();

    expect(ctrl.exportUserWords(), '{"version":1,"words":[]}');
    expect(find.text('已清除用户词库'), findsOneWidget);
    await tester.runAsync(() async => ctrl.dispose());
  });

  testWidgets('导出词库 JSON 复制到剪贴板', (tester) async {
    final copied = <String>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'Clipboard.setData') {
          copied.add((call.arguments as Map)['text'] as String);
        }
        return null;
      },
    );
    addTearDown(() => tester.binding.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, null));

    final ctrl = (await tester.runAsync(() async => await loadCtrl()))!;
    await tester.pumpWidget(MaterialApp(home: SettingsPage(controller: ctrl)));
    await tester.tap(find.text('导出词库 JSON'));
    await tester.pumpAndSettle();

    expect(copied, [ctrl.exportUserWords()]);
    expect(find.text('已复制到剪贴板'), findsOneWidget);
    await tester.runAsync(() async => ctrl.dispose());
  });
}
