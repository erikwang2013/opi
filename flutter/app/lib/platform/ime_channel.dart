import 'package:flutter/services.dart';

/// IME 通道抽象：生产走 MethodChannel，测试注入 fake。
abstract class ImeChannel {
  Future<void> commitText(String text);
  Future<void> deleteBackward();
  Future<void> performEnter();

  /// 输入目标切换通知（Kotlin 侧 onStartInput/onFinishInputView 触发）：
  /// Dart 应取消组合串、面板回 qwerty。传 null 注销。
  void setEditorChangedHandler(void Function()? handler);
}

class MethodChannelIme implements ImeChannel {
  static const MethodChannel _channel = MethodChannel('opi/ime');

  @override
  Future<void> commitText(String text) => _channel.invokeMethod('commitText', text);

  @override
  Future<void> deleteBackward() => _channel.invokeMethod('deleteBackward');

  @override
  Future<void> performEnter() => _channel.invokeMethod('performEnter');

  @override
  void setEditorChangedHandler(void Function()? handler) {
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'editorChanged') handler?.call();
      return null;
    });
  }
}
