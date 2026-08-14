import 'package:app/engine/engine_controller.dart';
import 'package:app/keyboards/key_button.dart';
import 'package:flutter/material.dart';

/// QWERTY 键盘：3 行字母（第 3 行含 ⇧）+ 底部功能行（中/英 / 123 / 空格 / ⌫ / ↵）。
class QwertyKeyboard extends StatelessWidget {
  const QwertyKeyboard({
    super.key,
    required this.onKey,
    required this.onSpace,
    required this.onBackspace,
    required this.onEnter,
    required this.onModeSwitch,
    this.onShift,
    this.onShiftLongPress,
    this.onNumber,
    this.onSymbolLongPress,
    this.shiftState,
    this.modeLabel = '中',
  });

  final ValueChanged<String> onKey;
  final VoidCallback onSpace;
  final VoidCallback onBackspace;
  final VoidCallback onEnter;
  final VoidCallback onModeSwitch;
  final VoidCallback? onShift;
  final VoidCallback? onShiftLongPress;
  final VoidCallback? onNumber;
  final VoidCallback? onSymbolLongPress;

  /// ⇧ 高亮状态（由调用方在 English 模式传入，Pinyin 模式传 off）。
  final ShiftState? shiftState;
  final String modeLabel;

  static const List<List<String>> _rows = [
    ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
    ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
    ['⇧', 'z', 'x', 'c', 'v', 'b', 'n', 'm'],
  ];

  @override
  Widget build(BuildContext context) {
    final shiftActive = shiftState != null && shiftState != ShiftState.off;
    return Column(
      children: [
        for (final row in _rows)
          Expanded(
            child: Row(
              children: [
                for (final ch in row)
                  if (ch == '⇧')
                    KeyButton(
                      '⇧',
                      onTap: onShift,
                      onLongPress: onShiftLongPress,
                      highlighted: shiftActive,
                    )
                  else
                    KeyButton(ch, onTap: () => onKey(ch)),
              ],
            ),
          ),
        Expanded(
          child: Row(
            children: [
              KeyButton(modeLabel, onTap: onModeSwitch),
              // 123 不挂长按：长按 500ms 吞 tap，导致面板打不开（符号经数字面板进入）
              KeyButton('123', onTap: onNumber),
              KeyButton('空格', flex: 3, onTap: onSpace),
              KeyButton('⌫', onTap: onBackspace),
              KeyButton('↵', onTap: onEnter),
            ],
          ),
        ),
      ],
    );
  }
}
