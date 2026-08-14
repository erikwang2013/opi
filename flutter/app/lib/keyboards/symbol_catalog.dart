import 'package:app/src/rust/api.dart';

/// 符号数据层：缓存 FFI 查询（每个查询仅一次），内存级最近使用（M5 不落盘）。
class SymbolCatalog {
  SymbolCatalog(this._api);

  final Api _api;

  List<ApiSymbolEntry>? _commonCache;
  List<ApiSymbolEntry>? _allCache;
  final List<String> _recent = [];

  static const int maxRecents = 50;

  /// 常用 = common 块并集（symbol_blocks → 逐块 symbols_in_block），按块序，按 text 去重。
  List<ApiSymbolEntry> get common {
    if (_commonCache == null) {
      final seen = <String>{};
      final out = <ApiSymbolEntry>[];
      for (final block in _api.symbolBlocks()) {
        for (final entry in _api.symbolsInBlock(id: block.id)) {
          if (seen.add(entry.text)) out.add(entry);
        }
      }
      _commonCache = out;
    }
    return _commonCache!;
  }

  /// 全部 = searchSymbols('')：空关键字时引擎返回全部条目（事实 3）。
  List<ApiSymbolEntry> get all {
    _allCache ??= _api.searchSymbols(keyword: '');
    return _allCache!;
  }

  /// 表情 = 全量按 emoji 过滤。
  List<ApiSymbolEntry> get emoji => all.where((e) => e.emoji).toList();

  List<ApiSymbolEntry> search(String q) {
    if (q.trim().isEmpty) return all;
    return _api.searchSymbols(keyword: q);
  }

  List<String> get recents => List.unmodifiable(_recent);

  void recordRecent(String text) {
    _recent.remove(text);
    _recent.insert(0, text);
    if (_recent.length > maxRecents) {
      _recent.removeRange(maxRecents, _recent.length);
    }
  }
}
