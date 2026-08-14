import 'package:app/engine/engine_controller.dart';
import 'package:app/keyboards/symbol_panel.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  setUpAll(() async {
    await RustLib.init();
  });

  Future<(EngineController, TextEditingController, FocusNode)> setup() async {
    final ctrl = await EngineController.load();
    return (ctrl, TextEditingController(), FocusNode());
  }

  Widget panel({
    required EngineController ctrl,
    required TextEditingController searchCtrl,
    required FocusNode focusNode,
    required ValueChanged<String> onCommit,
    bool searchActive = false,
  }) {
    return MaterialApp(
      home: Scaffold(
        body: SizedBox(
          height: 400,
          child: SymbolPanel(
            catalog: ctrl.symbols,
            onCommit: onCommit,
            onClose: () {},
            onBackToNumber: () {},
            searchController: searchCtrl,
            searchFocusNode: focusNode,
            searchActive: searchActive,
          ),
        ),
      ),
    );
  }

  testWidgets('Tab 数据：常用/表情/全部', (tester) async {
    final (ctrl, searchCtrl, focusNode) =
        (await tester.runAsync(() async => await setup()))!;
    addTearDown(() {
      searchCtrl.dispose();
      focusNode.dispose();
    });
    await tester.pumpWidget(panel(
      ctrl: ctrl,
      searchCtrl: searchCtrl,
      focusNode: focusNode,
      onCommit: (_) {},
    ));
    await tester.pump();

    // 常用 Tab
    expect(find.text(ctrl.symbols.common.first.text), findsOneWidget);

    // 表情 Tab：emoji 全量 + 最近使用（初始为空）
    await tester.tap(find.text('表情'));
    await tester.pump();
    expect(find.text(ctrl.symbols.emoji.first.text), findsWidgets);
    expect(ctrl.symbols.recents, isEmpty);

    // 全部 Tab
    await tester.tap(find.text('全部'));
    await tester.pump();
    expect(find.text(ctrl.symbols.all.first.text), findsOneWidget);
    await tester.runAsync(() async => ctrl.dispose());
  });

  testWidgets('点选提交 + 最近使用去重/前置/上限', (tester) async {
    final (ctrl, searchCtrl, focusNode) =
        (await tester.runAsync(() async => await setup()))!;
    addTearDown(() {
      searchCtrl.dispose();
      focusNode.dispose();
    });
    final committed = <String>[];
    await tester.pumpWidget(panel(
      ctrl: ctrl,
      searchCtrl: searchCtrl,
      focusNode: focusNode,
      onCommit: committed.add,
    ));
    await tester.pump();
    await tester.tap(find.text('表情'));
    await tester.pump();

    // fallback 内置符号仅 1 个 emoji（😄，生产 .opid 数据更大）
    final emoji = ctrl.symbols.emoji;
    expect(emoji.length, 1);
    await tester.tap(find.text(emoji.first.text));
    await tester.pump();
    expect(committed, [emoji.first.text]);
    expect(ctrl.symbols.recents, [emoji.first.text]);

    // 去重：再次点选（最近行 + 网格同时命中，取 first）
    await tester.tap(find.text(emoji.first.text).first);
    await tester.pump();
    expect(ctrl.symbols.recents, [emoji.first.text]);

    // 前置/上限（数据层直驱）
    ctrl.symbols.recordRecent('e2');
    ctrl.symbols.recordRecent('e3');
    ctrl.symbols.recordRecent(emoji.first.text);
    expect(ctrl.symbols.recents, [emoji.first.text, 'e3', 'e2']);
    for (var i = 0; i < 55; i++) {
      ctrl.symbols.recordRecent('f$i');
    }
    expect(ctrl.symbols.recents.length, 50);
    expect(ctrl.symbols.recents.first, 'f54');
    await tester.runAsync(() async => ctrl.dispose());
  });

  testWidgets('搜索：焦点态 + 关键字过滤（qwerty 程序化输入路径）', (tester) async {
    final (ctrl, searchCtrl, focusNode) =
        (await tester.runAsync(() async => await setup()))!;
    addTearDown(() {
      searchCtrl.dispose();
      focusNode.dispose();
    });
    final committed = <String>[];
    await tester.pumpWidget(panel(
      ctrl: ctrl,
      searchCtrl: searchCtrl,
      focusNode: focusNode,
      onCommit: committed.add,
      searchActive: true,
    ));
    await tester.pump();

    // 数据层：'he' 命中 ♥（M1 已核实）
    final results = ctrl.symbols.search('he');
    expect(results.any((e) => e.text == '♥'), isTrue);

    // 程序化改 controller 文本（qwerty 搜索盘路径）→ 250ms 防抖后出结果
    searchCtrl.text = 'he';
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pump();
    expect(find.text(results.first.text), findsWidgets);
    await tester.tap(find.text(results.first.text));
    expect(committed, [results.first.text]);

    // 表情 Tab 下搜索再过滤 emoji：用 😄 的关键字（数据驱动，fallback 仅 1 个 emoji）
    final emoji = ctrl.symbols.emoji;
    final kw = emoji.first.keywords.first;
    expect(ctrl.symbols.search(kw).where((e) => e.emoji), isNotEmpty);
    await tester.tap(find.text('表情'));
    await tester.pump();
    searchCtrl.text = kw;
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pump();
    expect(find.text(emoji.first.text), findsWidgets); // 网格显示过滤后的 emoji
    await tester.runAsync(() async => ctrl.dispose());
  });
}
