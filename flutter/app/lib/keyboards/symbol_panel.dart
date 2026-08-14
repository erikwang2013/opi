import 'dart:async';

import 'package:app/keyboards/key_button.dart';
import 'package:app/keyboards/symbol_catalog.dart';
import 'package:app/src/rust/api.dart';
import 'package:flutter/material.dart';

enum SymbolTab { common, emoji, all }

/// 符号面板：常用/表情/全部 Tab + 关键字搜索。
/// 搜索输入焦点联动 qwerty（IME 窗口内 TextField 不会唤起系统键盘），
/// 输入框 controller 由父级持有；此处监听其变化并 250ms 防抖出结果。
class SymbolPanel extends StatefulWidget {
  const SymbolPanel({
    super.key,
    required this.catalog,
    required this.onCommit,
    required this.onClose,
    required this.onBackToNumber,
    required this.searchController,
    required this.searchFocusNode,
    required this.searchActive,
  });

  final SymbolCatalog catalog;
  final ValueChanged<String> onCommit;
  final VoidCallback onClose;
  final VoidCallback onBackToNumber;
  final TextEditingController searchController;
  final FocusNode searchFocusNode;

  /// 焦点态：true 时下方显示 qwerty 字母盘供搜索输入。
  final bool searchActive;

  @override
  State<SymbolPanel> createState() => _SymbolPanelState();
}

class _SymbolPanelState extends State<SymbolPanel> {
  SymbolTab _tab = SymbolTab.common;
  Timer? _debounce;
  String _query = '';

  @override
  void initState() {
    super.initState();
    widget.searchController.addListener(_onInputChanged);
  }

  @override
  void dispose() {
    _debounce?.cancel();
    widget.searchController.removeListener(_onInputChanged);
    super.dispose();
  }

  /// 统一入口：TextField 键入与 qwerty 搜索盘程序化修改都经 controller 监听。
  void _onInputChanged() {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 250), () {
      if (mounted) setState(() => _query = widget.searchController.text.trim());
    });
  }

  void _commit(ApiSymbolEntry entry) {
    if (entry.emoji) widget.catalog.recordRecent(entry.text);
    widget.onCommit(entry.text);
  }

  String _tabLabel(SymbolTab t) => switch (t) {
        SymbolTab.common => '常用',
        SymbolTab.emoji => '表情',
        SymbolTab.all => '全部',
      };

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        _header(),
        _tabBar(),
        Expanded(child: _body()),
      ],
    );
  }

  Widget _header() {
    return Row(
      children: [
        KeyButton('ABC', flex: 2, onTap: widget.onClose),
        KeyButton('123', flex: 2, onTap: widget.onBackToNumber),
        Expanded(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4),
            child: TextField(
              controller: widget.searchController,
              focusNode: widget.searchFocusNode,
              textInputAction: TextInputAction.search,
              decoration: const InputDecoration(
                hintText: '搜索（拼音/英文）',
                isDense: true,
                border: OutlineInputBorder(),
                contentPadding: EdgeInsets.symmetric(horizontal: 8),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _tabBar() {
    return Row(
      children: [
        for (final tab in SymbolTab.values)
          Expanded(
            child: InkWell(
              onTap: () => setState(() => _tab = tab),
              child: Container(
                height: 36,
                color: _tab == tab ? Colors.grey.shade300 : Colors.transparent,
                alignment: Alignment.center,
                child: Text(_tabLabel(tab)),
              ),
            ),
          ),
      ],
    );
  }

  Widget _body() {
    if (widget.searchActive && _query.isNotEmpty) {
      final results = widget.catalog.search(_query);
      final shown =
          _tab == SymbolTab.emoji ? results.where((e) => e.emoji).toList() : results;
      return _grid(shown);
    }
    switch (_tab) {
      case SymbolTab.common:
        return _grid(widget.catalog.common);
      case SymbolTab.emoji:
        return Column(
          children: [
            if (widget.catalog.recents.isNotEmpty) _recentsRow(),
            Expanded(child: _grid(widget.catalog.emoji)),
          ],
        );
      case SymbolTab.all:
        return _grid(widget.catalog.all);
    }
  }

  Widget _recentsRow() {
    final recents = widget.catalog.recents;
    return SizedBox(
      height: 44,
      child: ListView(
        scrollDirection: Axis.horizontal,
        children: [
          for (final text in recents)
            InkWell(
              onTap: () => widget.onCommit(text),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 10),
                child: Center(child: Text(text, style: const TextStyle(fontSize: 22))),
              ),
            ),
        ],
      ),
    );
  }

  /// GridView.builder 惰性构建（全量条目数千，count 列表会全量构建）。
  Widget _grid(List<ApiSymbolEntry> entries) {
    return GridView.builder(
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(crossAxisCount: 8),
      itemCount: entries.length,
      itemBuilder: (context, i) {
        final entry = entries[i];
        return InkWell(
          onTap: () => _commit(entry),
          child: Center(
            child: Text(
              entry.text,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 20),
            ),
          ),
        );
      },
    );
  }
}
