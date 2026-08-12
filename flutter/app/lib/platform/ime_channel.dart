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
