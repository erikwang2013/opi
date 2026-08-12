import 'package:app/src/rust/api.dart';
import 'package:flutter/foundation.dart';

/// 单一状态源：封装 opi-ffi Api，UI 经 notifyListeners 订阅。
/// M4 键盘接入时由 ChangeNotifierProvider 提供（Riverpod）。
class EngineController extends ChangeNotifier {
  EngineController._(this._api);

  final Api _api;

  String buffer = '';
  ApiMode mode = ApiMode.pinyin;
  List<ApiCandidate> candidates = const [];

  static Future<EngineController> load() async {
    final api = await Api.loadFallback();
    final ctrl = EngineController._(api);
    ctrl.refresh();
    return ctrl;
  }

  void refresh() {
    buffer = _api.buffer();
    mode = _api.mode();
    candidates = _api.candidates(limit: BigInt.from(8));
    notifyListeners();
  }

  void input(String ch) {
    _api.inputKey(ch: ch);
    refresh();
  }

  void backspace() {
    _api.backspace();
    refresh();
  }

  void clear() {
    _api.clear();
    refresh();
  }

  String select(int index) {
    final text = _api.select(index: BigInt.from(index));
    refresh();
    return text;
  }

  void switchMode(ApiMode m) {
    _api.switchMode(mode: m);
    refresh();
  }

  void setShift(bool on) {
    _api.setShift(on_: on);
    refresh();
  }

  void inputSpace() {
    _api.inputSpace();
    refresh();
  }

  List<ApiSymbolEntry> searchSymbols(String keyword) =>
      _api.searchSymbols(keyword: keyword);
}
