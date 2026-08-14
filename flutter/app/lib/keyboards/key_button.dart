import 'package:flutter/material.dart';

/// 通用键组件：Material+InkWell，支持长按与高亮（⇧ 激活态用）。
class KeyButton extends StatelessWidget {
  const KeyButton(
    this.label, {
    super.key,
    this.onTap,
    this.onLongPress,
    this.flex = 1,
    this.highlighted = false,
    this.child,
  });

  final String label;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final int flex;
  final bool highlighted;
  final Widget? child;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      flex: flex,
      child: Padding(
        padding: const EdgeInsets.all(1),
        child: Material(
          color: highlighted ? Colors.blueGrey.shade600 : Colors.grey.shade300,
          borderRadius: BorderRadius.circular(6),
          child: InkWell(
            borderRadius: BorderRadius.circular(6),
            onTap: onTap,
            onLongPress: onLongPress,
            child: Center(
              child: child ??
                  Text(label, style: const TextStyle(fontSize: 18)),
            ),
          ),
        ),
      ),
    );
  }
}
