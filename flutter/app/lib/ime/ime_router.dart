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
