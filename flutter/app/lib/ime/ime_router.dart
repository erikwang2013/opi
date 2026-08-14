import 'package:app/engine/engine_controller.dart';
import 'package:app/platform/ime_channel.dart';
import 'package:app/src/rust/api.dart';

/// 按键分流：buffer 非空走引擎，buffer 空走通道直传。
class ImeRouter {
  ImeRouter(this.controller, this.channel);

  final EngineController controller;
  final ImeChannel channel;

  /// 面板提交统一入口（数字/符号/表情直传，不经引擎）。
  void commitText(String text) => channel.commitText(text);

  void handleKey(String ch) {
    if (controller.mode == ApiMode.english && controller.buffer.isEmpty) {
      // ⇧ 直传大写：直传路径绕过引擎，需 Dart 侧转大写（事实 4/7）
      if (controller.shiftState != ShiftState.off) {
        ch = ch.toUpperCase();
        controller.consumeSingleShift();
      }
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

  /// 屏内下标 → selectFromPage（翻页后的绝对下标）。
  void handleCandidate(int indexInPage) {
    final text = controller.selectFromPage(indexInPage);
    if (text.isNotEmpty) channel.commitText(text);
  }
}
