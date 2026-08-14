import 'dart:io';

import 'package:app/keyboards/symbol_catalog.dart';
import 'package:app/src/rust/api.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// ⇧ 状态：off / single（下个字母大写后自动复位）/ lock（持续大写）。
/// 引擎无 shift getter，状态全部由 Dart 维护（事实 4）。
enum ShiftState { off, single, lock }

/// 单一状态源：封装 opi-ffi Api，UI 经 notifyListeners 订阅。
class EngineController extends ChangeNotifier {
  EngineController._(this._api);

  final Api _api;

  String buffer = '';
  ApiMode mode = ApiMode.pinyin;
  List<ApiCandidate> candidates = const [];

  /// 候选翻页：引擎 candidates(limit) 无 offset，翻页纯客户端（M5）。
  static const int pageSize = 8;
  static const int fetchLimit = 64;
  int _candidatePage = 0;
  String? _lastQuery; // buffer 变化才重置页码（shift 等操作不重置）

  /// ⇧ 状态机。
  ShiftState shiftState = ShiftState.off;

  /// 符号数据层（缓存 FFI 查询，惰性加载）。
  late final SymbolCatalog symbols = SymbolCatalog(_api);

  /// 加载 luna 完整词库（asset → 应用目录 → FFI）。
  /// 失败回退内置 35 词词典：与 Rust load_or_fallback 语义一致，
  /// 同时让纯 dart 测试（无平台通道）保持可用。
  static Future<EngineController> load() async {
    Api api;
    try {
      api = await _loadLuna();
    } catch (e) {
      if (kDebugMode) debugPrint('OPI luna load failed ($e), using fallback dict');
      api = await Api.loadFallback();
    }
    final ctrl = EngineController._(api);
    ctrl.refresh();
    return ctrl;
  }

  static Future<Api> _loadLuna() async {
    // systemTemp 不引入插件依赖（path_provider 会带 jni 包，构建要求 NDK 28 且离线不可装）
    final path = '${Directory.systemTemp.path}/luna.opid';
    if (!await File(path).exists()) {
      final bytes = await rootBundle.load('assets/luna.opid');
      await File(path).writeAsBytes(bytes.buffer.asUint8List());
      if (kDebugMode) debugPrint('OPI copied luna.opid (${bytes.lengthInBytes} bytes)');
    }
    return Api.load(path: path);
  }

  void refresh() {
    buffer = _api.buffer();
    mode = _api.mode();
    candidates = _api.candidates(limit: BigInt.from(fetchLimit));
    if (buffer != _lastQuery) {
      _lastQuery = buffer;
      _candidatePage = 0;
    }
    // 候选数缩水（如退格）时钳制页码
    final maxPage = candidates.isEmpty ? 0 : (candidates.length - 1) ~/ pageSize;
    if (_candidatePage > maxPage) _candidatePage = maxPage;
    if (kDebugMode) {
      debugPrint('OPI refresh buffer="$buffer" mode=$mode page=$_candidatePage '
          'cands=${candidates.map((c) => c.text).toList()}');
    }
    notifyListeners();
  }

  void input(String ch) {
    final out = _api.inputKey(ch: ch);
    if (kDebugMode) debugPrint('OPI input("$ch") -> "$out"');
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
    if (kDebugMode) debugPrint('OPI switchMode -> $m');
    _api.switchMode(mode: m);
    refresh();
  }

  void setShift(bool on) {
    _api.setShift(on_: on);
    refresh();
  }

  String inputSpace() {
    final text = _api.inputSpace();
    refresh();
    return text;
  }

  List<ApiSymbolEntry> searchSymbols(String keyword) =>
      _api.searchSymbols(keyword: keyword);

  // ---- 候选翻页（M5）----

  int get candidatePage => _candidatePage;

  int get candidatePageCount => (candidates.length + pageSize - 1) ~/ pageSize;

  List<ApiCandidate> get pageCandidates {
    final start = _candidatePage * pageSize;
    if (start >= candidates.length) return const [];
    final end = (start + pageSize).clamp(0, candidates.length);
    return candidates.sublist(start, end);
  }

  void nextPage() {
    if (_candidatePage < candidatePageCount - 1) {
      _candidatePage++;
      notifyListeners();
    }
  }

  void prevPage() {
    if (_candidatePage > 0) {
      _candidatePage--;
      notifyListeners();
    }
  }

  /// 屏内下标 i → 绝对下标 page*8+i。
  String selectFromPage(int indexInPage) {
    return select(_candidatePage * pageSize + indexInPage);
  }

  // ---- ⇧ 状态机（M5）----

  void shiftTap() {
    if (shiftState == ShiftState.off) {
      shiftState = ShiftState.single;
      _api.setShift(on_: true);
    } else {
      shiftState = ShiftState.off;
      _api.setShift(on_: false);
    }
    // shift 不影响 buffer/candidates，仅通知 UI（也不重置候选页码）
    notifyListeners();
  }

  void shiftLongPress() {
    shiftState = ShiftState.lock;
    _api.setShift(on_: true);
    notifyListeners();
  }

  /// single 态消费后复位（lock 不受影响）。
  void consumeSingleShift() {
    if (shiftState == ShiftState.single) {
      shiftState = ShiftState.off;
      _api.setShift(on_: false);
      notifyListeners();
    }
  }

  // ---- 设置页透传（M5，作用于本实例；M6 SQLite 后跨实例）----

  bool get learnerEnabled => _api.learnerEnabled();

  void setLearner(bool on) => _api.setLearner(enabled: on);

  void clearUserWords() => _api.clearUserWords();

  String exportUserWords() => _api.exportUserWords();
}
