# M4 Android IME 集成 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 OPI 在 Android 上成为可用输入法——`OpiImeService`（InputMethodService）+ FlutterView 键盘进 IME 窗口，击键经 FFI 走 Rust 引擎，候选经 MethodChannel 提交到应用。

**Architecture:** Kotlin IME shell 只做三件通道方法（commitText/deleteBackward/performEnter），全部键盘/候选 UI 在 Dart 侧（独立 entrypoint `imeMain` + 独立 FlutterEngine）。Dart 侧 `ImeRouter` 做按键分流：buffer 非空走引擎，buffer 空直传通道。测试用 `ImeChannel` 抽象注入 fake，不触真通道。

**Tech Stack:** Android InputMethodService / FlutterView / FlutterEngine（DartExecutor entrypoint）/ MethodChannel / Riverpod 已有 EngineController / flutter_rust_bridge Api

**Spec:** [2026-08-12-opi-ime-android-m4-design.md](../specs/2026-08-12-opi-ime-android-m4-design.md)

**环境事实（已验证）：**
- `sdk.dir=/usr/lib/android-sdk`，Android SDK 存在 → `flutter build apk --debug` 可作验收
- 宿主 `flutter test` 能加载真实 Rust lib（M3 已证明：`RustLib.init()` + `Api.loadFallback()`）
- `com.example` 仅存在于 `android/app/build.gradle.kts`（2 行）与 `MainActivity.kt` 包名
- `Api.inputSpace()` 已返回 String（frb 签名），但 `EngineController.inputSpace()` 丢弃了返回值 → Task 1 修复

---

### Task 1: EngineController.inputSpace 返回提交文本

**Files:**
- Modify: `flutter/app/lib/engine/engine_controller.dart:60-63`
- Test: `flutter/app/test/engine_controller_test.dart`

- [ ] **Step 1: 写失败测试**

在 `flutter/app/test/engine_controller_test.dart` 的 `main()` 内、`controller drives buffer/mode/candidates` 测试之后追加：

```dart
  test('inputSpace returns committed text', () async {
    final ctrl = await EngineController.load();
    ctrl.input('w');
    ctrl.input('o');
    expect(ctrl.inputSpace(), '我'); // 当前返回 void，编译失败
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd flutter/app && flutter test test/engine_controller_test.dart`
Expected: FAIL —— `expect(ctrl.inputSpace(), '我')` 编译错误（void 不可比较）。

- [ ] **Step 3: 修改 EngineController.inputSpace 返回文本**

`flutter/app/lib/engine/engine_controller.dart`，把 `inputSpace()` 的 void 丢弃改为返回：

```dart
  String inputSpace() {
    final text = _api.inputSpace();
    refresh();
    return text;
  }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd flutter/app && flutter test test/engine_controller_test.dart`
Expected: PASS（全部测试，含新增 inputSpace 断言）。

- [ ] **Step 5: 提交**

```bash
git add flutter/app/lib/engine/engine_controller.dart flutter/app/test/engine_controller_test.dart
git commit -m "feat(flutter): EngineController.inputSpace returns committed text"
```

---

### Task 2: ImeChannel 抽象 + MethodChannel 实现

**Files:**
- Create: `flutter/app/lib/platform/ime_channel.dart`
- Create: `flutter/app/test/fake_ime_channel.dart`（共享测试替身）

- [ ] **Step 1: 创建通道抽象**

`flutter/app/lib/platform/ime_channel.dart`：

```dart
import 'package:flutter/services.dart';

/// IME 通道抽象：生产走 MethodChannel，测试注入 fake。
abstract class ImeChannel {
  Future<void> commitText(String text);
  Future<void> deleteBackward();
  Future<void> performEnter();
}

class MethodChannelIme implements ImeChannel {
  static const MethodChannel _channel = MethodChannel('opi/ime');

  @override
  Future<void> commitText(String text) => _channel.invokeMethod('commitText', text);

  @override
  Future<void> deleteBackward() => _channel.invokeMethod('deleteBackward');

  @override
  Future<void> performEnter() => _channel.invokeMethod('performEnter');
}
```

- [ ] **Step 2: 创建测试 fake**

`flutter/app/test/fake_ime_channel.dart`：

```dart
import 'package:app/platform/ime_channel.dart';

class FakeImeChannel implements ImeChannel {
  final commits = <String>[];
  int deleteCount = 0;
  int enterCount = 0;

  @override
  Future<void> commitText(String text) async => commits.add(text);

  @override
  Future<void> deleteBackward() async => deleteCount++;

  @override
  Future<void> performEnter() async => enterCount++;
}
```

- [ ] **Step 3: 运行现有测试确认无回归**

Run: `cd flutter/app && flutter analyze && flutter test`
Expected: 全绿（新文件纯接口，无新测试；fake 供 Task 3/6 使用）。

- [ ] **Step 4: 提交**

```bash
git add flutter/app/lib/platform/ime_channel.dart flutter/app/test/fake_ime_channel.dart
git commit -m "feat(flutter): ImeChannel abstraction with fake for tests"
```

---

### Task 3: ImeRouter 按键分流

**Files:**
- Create: `flutter/app/lib/ime/ime_router.dart`
- Test: `flutter/app/test/ime_router_test.dart`

- [ ] **Step 1: 写失败测试**

`flutter/app/test/ime_router_test.dart`：

```dart
import 'package:app/engine/engine_controller.dart';
import 'package:app/ime/ime_router.dart';
import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fake_ime_channel.dart';

void main() {
  setUpAll(() async {
    await RustLib.init();
  });

  Future<(EngineController, ImeRouter, FakeImeChannel)> setup() async {
    final ctrl = await EngineController.load();
    final channel = FakeImeChannel();
    return (ctrl, ImeRouter(ctrl, channel), channel);
  }

  test('空 buffer：空格/退格/回车直传通道', () async {
    final (ctrl, router, channel) = await setup();
    router.handleSpace();
    router.handleBackspace();
    router.handleEnter();
    expect(channel.commits, [' ']);
    expect(channel.deleteCount, 1);
    expect(channel.enterCount, 1);
    ctrl.dispose();
  });

  test('拼音：字母进缓冲，空格提交首候选', () async {
    final (ctrl, router, channel) = await setup();
    router.handleKey('w');
    router.handleKey('o');
    expect(ctrl.buffer, 'wo');
    router.handleSpace();
    expect(channel.commits, ['我']);
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });

  test('拼音：退格缩短缓冲，回车提交首候选', () async {
    final (ctrl, router, channel) = await setup();
    router.handleKey('w');
    router.handleKey('o');
    router.handleBackspace();
    expect(ctrl.buffer, 'w');
    router.handleKey('o');
    router.handleEnter();
    expect(channel.commits, ['我']);
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });

  test('英文模式：空缓冲字母直传', () async {
    final (ctrl, router, channel) = await setup();
    ctrl.switchMode(ApiMode.english);
    router.handleKey('a');
    expect(channel.commits, ['a']);
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });

  test('候选选择提交所选文本', () async {
    final (ctrl, router, channel) = await setup();
    router.handleKey('w');
    router.handleKey('o');
    router.handleCandidate(0);
    expect(channel.commits, ['我']);
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd flutter/app && flutter test test/ime_router_test.dart`
Expected: FAIL —— `import 'package:app/ime/ime_router.dart'` 不存在（Uri 编译错误）。

- [ ] **Step 3: 实现 ImeRouter**

`flutter/app/lib/ime/ime_router.dart`：

```dart
import 'package:app/engine/engine_controller.dart';
import 'package:app/platform/ime_channel.dart';
import 'package:app/src/rust/api.dart';

/// 按键分流：buffer 非空走引擎，buffer 空走通道直传。
class ImeRouter {
  ImeRouter(this.controller, this.channel);

  final EngineController controller;
  final ImeChannel channel;

  void handleKey(String ch) {
    if (controller.mode == ApiMode.english && controller.buffer.isEmpty) {
      channel.commitText(ch);
      return;
    }
    controller.input(ch);
  }

  void handleSpace() {
    if (controller.buffer.isNotEmpty) {
      final text = controller.inputSpace();
      if (text.isNotEmpty) channel.commitText(text);
    } else {
      channel.commitText(' ');
    }
  }

  void handleBackspace() {
    if (controller.buffer.isNotEmpty) {
      controller.backspace();
    } else {
      channel.deleteBackward();
    }
  }

  void handleEnter() {
    if (controller.buffer.isNotEmpty) {
      final text = controller.select(0);
      if (text.isNotEmpty) channel.commitText(text);
    } else {
      channel.performEnter();
    }
  }

  void handleCandidate(int index) {
    final text = controller.select(index);
    if (text.isNotEmpty) channel.commitText(text);
  }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd flutter/app && flutter test test/ime_router_test.dart`
Expected: PASS（5 个测试）。

- [ ] **Step 5: 提交**

```bash
git add flutter/app/lib/ime/ime_router.dart flutter/app/test/ime_router_test.dart
git commit -m "feat(flutter): ImeRouter key routing (engine vs channel)"
```

---

### Task 4: QwertyKeyboard widget

**Files:**
- Create: `flutter/app/lib/keyboards/qwerty.dart`
- Test: `flutter/app/test/qwerty_keyboard_test.dart`

- [ ] **Step 1: 写失败测试**

`flutter/app/test/qwerty_keyboard_test.dart`：

```dart
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
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd flutter/app && flutter test test/qwerty_keyboard_test.dart`
Expected: FAIL —— `import 'package:app/keyboards/qwerty.dart'` 不存在。

- [ ] **Step 3: 实现 QwertyKeyboard**

`flutter/app/lib/keyboards/qwerty.dart`：

```dart
import 'package:flutter/material.dart';

/// 最小 QWERTY 键盘：3 行字母 + 底部功能行（🌐 / 123 / 空格 / ⌫ / ↵）。
class QwertyKeyboard extends StatelessWidget {
  const QwertyKeyboard({
    super.key,
    required this.onKey,
    required this.onSpace,
    required this.onBackspace,
    required this.onEnter,
    required this.onModeSwitch,
  });

  final ValueChanged<String> onKey;
  final VoidCallback onSpace;
  final VoidCallback onBackspace;
  final VoidCallback onEnter;
  final VoidCallback onModeSwitch;

  static const List<List<String>> _rows = [
    ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
    ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
    ['z', 'x', 'c', 'v', 'b', 'n', 'm'],
  ];

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        for (final row in _rows)
          Row(
            children: [
              for (final ch in row) _Key(ch, onTap: () => onKey(ch)),
            ],
          ),
        Row(
          children: [
            _Key('🌐', onTap: onModeSwitch),
            _Key('123'), // M5 生效，M4 占位
            _Key('空格', flex: 5, onTap: onSpace),
            _Key('⌫', onTap: onBackspace),
            _Key('↵', onTap: onEnter),
          ],
        ),
      ],
    );
  }
}

class _Key extends StatelessWidget {
  const _Key(this.label, {this.onTap, this.flex = 1});

  final String label;
  final VoidCallback? onTap;
  final int flex;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      flex: flex,
      child: Padding(
        padding: const EdgeInsets.all(2),
        child: Material(
          color: Colors.grey.shade300,
          borderRadius: BorderRadius.circular(6),
          child: InkWell(
            borderRadius: BorderRadius.circular(6),
            onTap: onTap,
            child: Center(
              child: Text(label, style: const TextStyle(fontSize: 20)),
            ),
          ),
        ),
      ),
    );
  }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd flutter/app && flutter test test/qwerty_keyboard_test.dart`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add flutter/app/lib/keyboards/qwerty.dart flutter/app/test/qwerty_keyboard_test.dart
git commit -m "feat(flutter): minimal QWERTY keyboard widget"
```

---

### Task 5: CandidateBar widget

**Files:**
- Create: `flutter/app/lib/candidates/candidate_bar.dart`
- Test: `flutter/app/test/candidate_bar_test.dart`

- [ ] **Step 1: 写失败测试**

`flutter/app/test/candidate_bar_test.dart`：

```dart
import 'package:app/candidates/candidate_bar.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('显示缓冲与候选，点击回调索引', (tester) async {
    await RustLib.init();
    final ctrl = await EngineController.load();
    ctrl.input('w');
    ctrl.input('o');
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
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd flutter/app && flutter test test/candidate_bar_test.dart`
Expected: FAIL —— `import 'package:app/candidates/candidate_bar.dart'` 不存在。

- [ ] **Step 3: 实现 CandidateBar**

`flutter/app/lib/candidates/candidate_bar.dart`：

```dart
import 'package:flutter/material.dart';

import 'package:app/engine/engine_controller.dart';

/// 候选栏：拼音缓冲 + 候选 top-8，点击选择（无 composing，候选栏即组合区）。
class CandidateBar extends StatelessWidget {
  const CandidateBar({super.key, required this.controller, required this.onTap});

  final EngineController controller;
  final ValueChanged<int> onTap;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 56,
      color: Colors.grey.shade200,
      child: Row(
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: Text(
              controller.buffer,
              style: const TextStyle(fontSize: 18, color: Colors.black54),
            ),
          ),
          Expanded(
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  for (var i = 0; i < controller.candidates.length; i++)
                    _Candidate(
                      controller.candidates[i].text,
                      onTap: () => onTap(i),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _Candidate extends StatelessWidget {
  const _Candidate(this.text, {required this.onTap});

  final String text;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 16),
        child: Text(text, style: const TextStyle(fontSize: 20)),
      ),
    );
  }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd flutter/app && flutter test test/candidate_bar_test.dart`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add flutter/app/lib/candidates/candidate_bar.dart flutter/app/test/candidate_bar_test.dart
git commit -m "feat(flutter): candidate bar (buffer + top-8 candidates)"
```

---

### Task 6: imeMain entrypoint + ImeApp/ImeScreen

**Files:**
- Create: `flutter/app/lib/ime/ime_main.dart`
- Test: `flutter/app/test/ime_main_test.dart`

- [ ] **Step 1: 写失败测试**

`flutter/app/test/ime_main_test.dart`：

```dart
import 'package:app/candidates/candidate_bar.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/ime/ime_main.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fake_ime_channel.dart';

void main() {
  testWidgets('全流程：输入 → 候选 → 点击提交经通道', (tester) async {
    await RustLib.init();
    final ctrl = await EngineController.load();
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));

    await tester.tap(find.text('w'));
    await tester.pump();
    await tester.tap(find.text('o'));
    await tester.pump();
    expect(ctrl.buffer, 'wo');

    await tester.tap(find.text('我'));
    await tester.pump();
    expect(channel.commits, ['我']);
    expect(ctrl.buffer, '');

    // 空 buffer 空格直传
    await tester.tap(find.text('空格'));
    await tester.pump();
    expect(channel.commits, ['我', ' ']);
    ctrl.dispose();
  });

  testWidgets('英文模式隐藏候选栏，字母直传', (tester) async {
    await RustLib.init();
    final ctrl = await EngineController.load();
    final channel = FakeImeChannel();
    await tester.pumpWidget(ImeApp(controller: ctrl, channel: channel));

    await tester.tap(find.text('🌐'));
    await tester.pump();
    expect(find.byType(CandidateBar), findsNothing);
    await tester.tap(find.text('a'));
    await tester.pump();
    expect(channel.commits, ['a']);
    expect(ctrl.buffer, '');
    ctrl.dispose();
  });
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd flutter/app && flutter test test/ime_main_test.dart`
Expected: FAIL —— `import 'package:app/ime/ime_main.dart'` 不存在。

- [ ] **Step 3: 实现 imeMain + ImeApp**

`flutter/app/lib/ime/ime_main.dart`：

```dart
import 'package:flutter/material.dart';

import 'package:app/candidates/candidate_bar.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/ime/ime_router.dart';
import 'package:app/keyboards/qwerty.dart';
import 'package:app/platform/ime_channel.dart';
import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';

/// IME 独立 entrypoint（M4：Flutter 键盘进 IME 窗口）。
Future<void> imeMain() async {
  await RustLib.init();
  final controller = await EngineController.load();
  runApp(ImeApp(controller: controller, channel: MethodChannelIme()));
}

class ImeApp extends StatelessWidget {
  const ImeApp({super.key, required this.controller, required this.channel});

  final EngineController controller;
  final ImeChannel channel;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      home: ImeScreen(controller: controller, channel: channel),
    );
  }
}

class ImeScreen extends StatefulWidget {
  const ImeScreen({super.key, required this.controller, required this.channel});

  final EngineController controller;
  final ImeChannel channel;

  @override
  State<ImeScreen> createState() => _ImeScreenState();
}

class _ImeScreenState extends State<ImeScreen> {
  late final ImeRouter _router = ImeRouter(widget.controller, widget.channel);

  void _toggleMode() {
    widget.controller.switchMode(
      widget.controller.mode == ApiMode.pinyin ? ApiMode.english : ApiMode.pinyin,
    );
  }

  @override
  Widget build(BuildContext context) {
    final height = MediaQuery.sizeOf(context).width * 0.42; // 固定高度：宽 × 0.42
    return Scaffold(
      backgroundColor: Colors.white,
      body: SizedBox(
        height: height,
        child: ListenableBuilder(
          listenable: widget.controller,
          builder: (context, _) => Column(
            children: [
              if (widget.controller.mode != ApiMode.english)
                CandidateBar(
                  controller: widget.controller,
                  onTap: _router.handleCandidate,
                ),
              Expanded(
                child: QwertyKeyboard(
                  onKey: _router.handleKey,
                  onSpace: _router.handleSpace,
                  onBackspace: _router.handleBackspace,
                  onEnter: _router.handleEnter,
                  onModeSwitch: _toggleMode,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd flutter/app && flutter test test/ime_main_test.dart`
Expected: PASS（2 个 widget 测试）。

- [ ] **Step 5: 运行全量 Dart 测试 + analyze 确认无回归**

Run: `cd flutter/app && flutter analyze && flutter test`
Expected: 全绿。

- [ ] **Step 6: 提交**

```bash
git add flutter/app/lib/ime/ime_main.dart flutter/app/test/ime_main_test.dart
git commit -m "feat(flutter): IME entrypoint with keyboard + candidate bar"
```

---

### Task 7: 包名迁移 com.example.app → io.opi.input

**Files:**
- Modify: `flutter/app/android/app/build.gradle.kts:8,19`
- Create: `flutter/app/android/app/src/main/kotlin/io/opi/input/MainActivity.kt`
- Delete: `flutter/app/android/app/src/main/kotlin/com/example/app/MainActivity.kt`

- [ ] **Step 1: 改 build.gradle.kts**

`flutter/app/android/app/build.gradle.kts` 两处：

```kotlin
    namespace = "io.opi.input"
```

```kotlin
        applicationId = "io.opi.input"
```

- [ ] **Step 2: 新建 MainActivity**

`flutter/app/android/app/src/main/kotlin/io/opi/input/MainActivity.kt`：

```kotlin
package io.opi.input

import io.flutter.embedding.android.FlutterActivity

class MainActivity : FlutterActivity()
```

- [ ] **Step 3: 删除旧文件**

Run: `git rm flutter/app/android/app/src/main/kotlin/com/example/app/MainActivity.kt`

- [ ] **Step 4: 验证无残留引用**

Run: `grep -rn "com.example" flutter/app/android --include="*.kts" --include="*.kt" --include="*.xml" --include="*.properties" || echo CLEAN`
Expected: 无输出（或仅 `.idea` 非构建文件）。

- [ ] **Step 5: 提交**

```bash
git add -A flutter/app/android
git commit -m "refactor(android): rename package to io.opi.input"
```

---

### Task 8: OpiImeService + method.xml + manifest IME 声明

**Files:**
- Create: `flutter/app/android/app/src/main/kotlin/io/opi/input/OpiImeService.kt`
- Create: `flutter/app/android/app/src/main/res/xml/method.xml`
- Modify: `flutter/app/android/app/src/main/AndroidManifest.xml`

- [ ] **Step 1: 新建 OpiImeService**

`flutter/app/android/app/src/main/kotlin/io/opi/input/OpiImeService.kt`：

```kotlin
package io.opi.input

import android.inputmethodservice.InputMethodService
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import io.flutter.embedding.android.FlutterView
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.dart.DartExecutor
import io.flutter.injector.FlutterInjector
import io.flutter.plugin.common.MethodChannel

/** OPI IME 宿主：FlutterView 作为输入视图，Dart imeMain entrypoint。 */
class OpiImeService : InputMethodService() {
    private var flutterEngine: FlutterEngine? = null
    private var channel: MethodChannel? = null

    override fun onCreateInputView(): View {
        val engine = FlutterEngine(this)
        val entrypoint = DartExecutor.DartEntrypoint(
            FlutterInjector.instance().flutterLoader().findAppBundlePath(),
            "imeMain"
        )
        engine.dartExecutor.executeDartEntrypoint(entrypoint)

        val view = FlutterView(
            this,
            FlutterView.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
        )
        view.attachToFlutterEngine(engine)

        channel = MethodChannel(engine.dartExecutor.binaryMessenger, "opi/ime")
        channel?.setMethodCallHandler { call, result ->
            when (call.method) {
                "commitText" -> {
                    currentInputConnection?.commitText(call.arguments as String, 1)
                    result.success(null)
                }
                "deleteBackward" -> {
                    currentInputConnection?.deleteSurroundingText(1, 0)
                    result.success(null)
                }
                "performEnter" -> {
                    currentInputConnection?.performEditorAction(EditorInfo.IME_ACTION_SEND)
                    result.success(null)
                }
                else -> result.notImplemented()
            }
        }

        flutterEngine = engine
        return view
    }

    override fun onDestroy() {
        flutterEngine?.destroy()
        flutterEngine = null
        channel = null
        super.onDestroy()
    }
}
```

- [ ] **Step 2: 新建 method.xml**

`flutter/app/android/app/src/main/res/xml/method.xml`：

```xml
<?xml version="1.0" encoding="utf-8"?>
<input-method xmlns:android="http://schemas.android.com/apk/res/android"
    android:settingsActivity="io.opi.input.MainActivity" />
```

- [ ] **Step 3: 修改 AndroidManifest.xml**

`flutter/app/android/app/src/main/AndroidManifest.xml`：
- `<application android:label="app"` → `android:label="OPI IME"`
- 在 `<!-- Don't delete the meta-data below. -->` 注释之前插入：

```xml
        <service
            android:name=".OpiImeService"
            android:label="OPI IME"
            android:exported="true"
            android:permission="android.permission.BIND_INPUT_METHOD">
            <intent-filter>
                <action android:name="android.view.InputMethod" />
            </intent-filter>
            <meta-data
                android:name="android.view.im"
                android:resource="@xml/method" />
        </service>
```

- [ ] **Step 4: 验证源码声明**

Run: `grep -n "OpiImeService\|BIND_INPUT_METHOD\|android.view.im" flutter/app/android/app/src/main/AndroidManifest.xml`
Expected: 3 处匹配。

- [ ] **Step 5: 提交**

```bash
git add flutter/app/android/app/src/main/kotlin/io/opi/input/OpiImeService.kt flutter/app/android/app/src/main/res/xml/method.xml flutter/app/android/app/src/main/AndroidManifest.xml
git commit -m "feat(android): OpiImeService InputMethodService with FlutterView"
```

---

### Task 9: 门禁 + APK 构建验收

**Files:** 无（验收任务）

- [ ] **Step 1: Rust 门禁**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: 全绿（零警告）。

- [ ] **Step 2: Dart 门禁**

Run: `cd flutter/app && flutter analyze && flutter test`
Expected: 全绿。

- [ ] **Step 3: 构建 debug APK**

Run: `cd flutter/app && flutter build apk --debug`
Expected: `✓ Built build/app/outputs/flutter-apk/app-debug.apk`（约 1.6MB+）。
注意：首次构建会下载 Gradle 依赖并让 cargokit 编译 Rust 到 Android targets（arm64-v8a / armeabi-v7a / x86_64），可能超过 10 分钟——用 run_in_background 或拉长超时。若因网络/rustup target 缺失失败：记录为 spec 偏差（追加到设计文档「实现偏差」节），确认源码无误后仍提交。

- [ ] **Step 4: 验证 APK 包名**

Run:

```bash
AAPT=$(find /usr/lib/android-sdk/build-tools -name aapt | sort | tail -1)
"$AAPT" dump badging flutter/app/build/app/outputs/flutter-apk/app-debug.apk | grep -E "^package:|launchable-activity"
```

Expected:

```
package: name='io.opi.input' ...
launchable-activity: name='io.opi.input.MainActivity' ...
```

- [ ] **Step 5: 记录验收结果**

在 `docs/superpowers/specs/2026-08-12-opi-ime-android-m4-design.md` 末尾追加「实现偏差（2026-08-12）」节，记录：真机验收未执行（无设备，列为待办）、构建耗时/网络问题（如有）、其他与设计的偏离。有偏离才追加；无偏离则跳过本步。

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "build(android): M4 APK builds with io.opi.input package"
```

---

## 验收映射（spec §6）

| 验收项 | 任务 |
|---|---|
| 1. `flutter build apk --debug` 成功 | Task 9 Step 3 |
| 2. manifest 含 IME service + BIND_INPUT_METHOD + method.xml，包名 io.opi.input | Task 7 + Task 8 + Task 9 Step 4 |
| 3. cargo 门禁全绿 | Task 9 Step 1 |
| 4. flutter test / analyze 全绿 | Task 1/6/9 Step 2 |
| 5. 真机验收推迟 | Task 9 Step 5（记录偏差） |
