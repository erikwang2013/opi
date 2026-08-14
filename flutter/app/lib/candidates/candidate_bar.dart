import 'package:flutter/material.dart';

import 'package:app/engine/engine_controller.dart';
import 'package:app/src/rust/api.dart';

/// 候选栏：拼音缓冲 + 候选每屏 8 个，点击选择；页数>1 时显示 ‹ n/m › 翻页。
/// 无状态：数据与翻页状态均在 EngineController（单一状态源）。
class CandidateBar extends StatelessWidget {
  const CandidateBar({super.key, required this.controller, required this.onTap});

  final EngineController controller;
  final ValueChanged<int> onTap;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final pageCount = controller.candidatePageCount;
        final candidates = controller.pageCandidates;
        // english 模式：候选栏退化为模式条，切换中/英有明确区域反馈
        if (controller.mode == ApiMode.english) {
          return Container(
            height: 44,
            color: Colors.grey.shade200,
            child: Row(
              children: [
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: Text(
                    'EN',
                    style: TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.bold,
                      color: Colors.blueGrey.shade700,
                    ),
                  ),
                ),
                Text(
                  '字母直接上屏',
                  style: TextStyle(fontSize: 13, color: Colors.grey.shade600),
                ),
              ],
            ),
          );
        }
        return Container(
          height: 44,
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
                      for (var i = 0; i < candidates.length; i++)
                        _Candidate(
                          candidates[i].text,
                          onTap: () => onTap(i),
                        ),
                      // 拼音无候选：给出可见反馈，避免"打字无反应"错觉
                      if (candidates.isEmpty && controller.buffer.isNotEmpty)
                        Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 12),
                          child: Text(
                            '无匹配',
                            style: TextStyle(
                                fontSize: 13, color: Colors.grey.shade500),
                          ),
                        ),
                    ],
                  ),
                ),
              ),
              if (pageCount > 1) ...[
                IconButton(
                  icon: const Icon(Icons.chevron_left),
                  visualDensity: VisualDensity.compact,
                  onPressed: controller.prevPage,
                ),
                Text(
                  '${controller.candidatePage + 1}/$pageCount',
                  style: const TextStyle(fontSize: 13),
                ),
                IconButton(
                  icon: const Icon(Icons.chevron_right),
                  visualDensity: VisualDensity.compact,
                  onPressed: controller.nextPage,
                ),
              ],
            ],
          ),
        );
      },
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
      // 16×2 垂直 padding + 28 行高 ≈ 60dp，溢出 44dp 栏高且点按区超可视区
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        child: Text(text, style: const TextStyle(fontSize: 20)),
      ),
    );
  }
}
