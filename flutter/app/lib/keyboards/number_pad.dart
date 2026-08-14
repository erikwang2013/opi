import 'package:app/keyboards/key_button.dart';
import 'package:flutter/material.dart';

/// 数字面板：Gboard 风格 5 行。数字/标点直接提交，不经过引擎
/// （引擎 Number 模式无可用提交路径：select 恒返回空，事实 1）。
class NumberPad extends StatelessWidget {
  const NumberPad({
    super.key,
    required this.onKey,
    required this.onSymbol,
    required this.onLetters,
    required this.onSpace,
    required this.onBackspace,
    required this.onEnter,
  });

  final ValueChanged<String> onKey;
  final VoidCallback onSymbol;
  final VoidCallback onLetters;
  final VoidCallback onSpace;
  final VoidCallback onBackspace;
  final VoidCallback onEnter;

  static const List<List<String>> _rows = [
    ['1', '2', '3'],
    ['4', '5', '6'],
    ['7', '8', '9'],
    [',', '0', '.'],
  ];

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        for (final row in _rows)
          Expanded(
            child: Row(
              children: [
                for (final ch in row) KeyButton(ch, onTap: () => onKey(ch)),
              ],
            ),
          ),
        Expanded(
          child: Row(
            children: [
              KeyButton('ABC', onTap: onLetters),
              KeyButton('?123', onTap: onSymbol),
              KeyButton('空格', flex: 5, onTap: onSpace),
              KeyButton('⌫', onTap: onBackspace),
              KeyButton('↵', onTap: onEnter),
            ],
          ),
        ),
      ],
    );
  }
}
