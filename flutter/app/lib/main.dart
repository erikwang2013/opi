import 'package:flutter/material.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/ime/ime_main.dart' show imeMain;
import 'package:app/settings/settings_page.dart';
import 'package:app/src/rust/frb_generated.dart';

Future<void> main() async {
  // imeMain 由 OpiImeService 经 DartExecutor 按名加载；引用它使其编入 debug
  // kernel（否则 "Could not resolve main entrypoint function"，键盘全黑）。
  assert(imeMain is Function);
  await RustLib.init();
  final controller = await EngineController.load();
  runApp(MyApp(controller: controller));
}

class MyApp extends StatelessWidget {
  const MyApp({super.key, required this.controller});

  final EngineController controller;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: SettingsPage(controller: controller),
    );
  }
}
