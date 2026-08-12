import 'package:flutter/material.dart';

import 'package:app/engine/engine_controller.dart';

/// 候选栏：拼音缓冲 + 候选 top-8，点击选择（无 composing，候选栏即组合区）。
class CandidateBar extends StatelessWidget {
  const CandidateBar({super.key, required this.controller, required this.onTap});

  final EngineController controller;
  final ValueChanged<int> onTap;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 56,
      color: Colors.grey.shade200,
      child: Row(
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: Text(
              controller.buffer,
              style: const TextStyle(fontSize: 18, color: Colors.black54),
            ),
          ),
          Expanded(
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  for (var i = 0; i < controller.candidates.length; i++)
                    _Candidate(
                      controller.candidates[i].text,
                      onTap: () => onTap(i),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _Candidate extends StatelessWidget {
  const _Candidate(this.text, {required this.onTap});

  final String text;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 16),
        child: Text(text, style: const TextStyle(fontSize: 20)),
      ),
    );
  }
}
