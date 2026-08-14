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

  void Function()? editorChangedHandler;

  @override
  void setEditorChangedHandler(void Function()? handler) {
    editorChangedHandler = handler;
  }
}
