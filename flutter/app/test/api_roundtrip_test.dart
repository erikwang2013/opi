import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('type round trips and error mapping', () async {
    await RustLib.init();
    final api = await Api.loadFallback();

    // 初始状态
    expect(api.buffer(), '');
    expect(api.mode(), ApiMode.pinyin);
    expect(api.learnerEnabled(), true);

    // String / usize→int / u64→int / 枚举 / 结构体往返
    api.inputKey(ch: 'w');
    api.inputKey(ch: 'o');
    expect(api.buffer(), 'wo');
    final cands = api.candidates(limit: BigInt.from(3));
    expect(cands.length, greaterThan(0));
    expect(cands.first.text, '我');
    expect(cands.first.kind, ApiCandidateKind.hanzi);
    expect(cands.first.score, greaterThan(BigInt.zero));

    // 错误映射：越界 select → 空串；非法输入 → 空串
    expect(api.select(index: BigInt.from(999)), '');
    expect(api.inputKey(ch: ''), '');
    expect(api.inputKey(ch: 'ab'), '');
    expect(api.buffer(), 'wo');

    // 模式枚举映射
    api.switchMode(mode: ApiMode.english);
    expect(api.mode(), ApiMode.english);
    api.switchMode(mode: ApiMode.pinyin);

    // 符号：Vec<String> / 结构体字段
    final hits = api.searchSymbols(keyword: 'he');
    expect(hits.any((s) => s.text == '♥'), true);
    final blocks = api.symbolBlocks();
    expect(blocks, isNotEmpty);
    expect(blocks.first.common, isA<bool>());
    expect(api.symbolsInBlock(id: blocks.first.id), isNotEmpty);

    // 学习数据 JSON 往返
    api.clearUserWords();
    expect(api.exportUserWords(), '{"version":1,"words":[]}');
    api.setLearner(enabled: false);
    expect(api.learnerEnabled(), false);
  });
}
