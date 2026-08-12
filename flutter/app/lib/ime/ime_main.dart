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
