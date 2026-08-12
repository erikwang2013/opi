import 'package:flutter/material.dart';

/// 最小 QWERTY 键盘：3 行字母 + 底部功能行（🌐 / 123 / 空格 / ⌫ / ↵）。
class QwertyKeyboard extends StatelessWidget {
  const QwertyKeyboard({
    super.key,
    required this.onKey,
    required this.onSpace,
    required this.onBackspace,
    required this.onEnter,
    required this.onModeSwitch,
  });

  final ValueChanged<String> onKey;
  final VoidCallback onSpace;
  final VoidCallback onBackspace;
  final VoidCallback onEnter;
  final VoidCallback onModeSwitch;

  static const List<List<String>> _rows = [
    ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
    ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
    ['z', 'x', 'c', 'v', 'b', 'n', 'm'],
  ];

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        for (final row in _rows)
          Expanded(
            child: Row(
              children: [
                for (final ch in row) _Key(ch, onTap: () => onKey(ch)),
              ],
            ),
          ),
        Expanded(
          child: Row(
            children: [
              _Key('🌐', onTap: onModeSwitch),
              _Key('123'), // M5 生效，M4 占位
              _Key('空格', flex: 5, onTap: onSpace),
              _Key('⌫', onTap: onBackspace),
              _Key('↵', onTap: onEnter),
            ],
          ),
        ),
      ],
    );
  }
}

class _Key extends StatelessWidget {
  const _Key(this.label, {this.onTap, this.flex = 1});

  final String label;
  final VoidCallback? onTap;
  final int flex;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      flex: flex,
      child: Padding(
        padding: const EdgeInsets.all(1),
        child: Material(
          color: Colors.grey.shade300,
          borderRadius: BorderRadius.circular(6),
          child: InkWell(
            borderRadius: BorderRadius.circular(6),
            onTap: onTap,
            child: Center(
              child: Text(label, style: const TextStyle(fontSize: 18)),
            ),
          ),
        ),
      ),
    );
  }
}
