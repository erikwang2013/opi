import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('full pinyin flow: input, candidates, select, space', () async {
    await RustLib.init();
    final api = await Api.loadFallback();

    // 拼音输入 → 候选 → 选词提交
    for (final c in ['w', 'o']) {
      api.inputKey(ch: c);
    }
    expect(api.buffer(), 'wo');
    expect(api.candidates(limit: BigInt.from(8)).first.text, '我');
    expect(api.select(index: BigInt.zero), '我'); // select 返回已提交的候选词
    expect(api.buffer(), '');

    // 空格键：拼音模式提交首候选
    for (final c in ['n', 'i']) {
      api.inputKey(ch: c);
    }
    expect(api.buffer(), 'ni');
    api.inputSpace(); // 空格 → 提交首候选
    expect(api.buffer(), '');
  });

  test('mode switching and english commit', () async {
    final api = await Api.loadFallback();
    api.switchMode(mode: ApiMode.english);
    expect(api.mode(), ApiMode.english);

    for (final c in ['a', 'b', 'c']) {
      api.inputKey(ch: c);
    }
    expect(api.buffer(), 'abc');
    expect(api.inputSpace(), 'abc'); // 英文模式空格提交缓冲并返回
    expect(api.buffer(), '');
    api.switchMode(mode: ApiMode.pinyin);
    expect(api.mode(), ApiMode.pinyin);
  });

  test('symbols: block browse and keyword search', () async {
    final api = await Api.loadFallback();
    final blocks = api.symbolBlocks();
    expect(blocks, isNotEmpty);
    final all = <String>[];
    for (final b in blocks) {
      all.addAll(api.symbolsInBlock(id: b.id).map((s) => s.text));
    }
    expect(all, contains('♥'));
    expect(all, contains('😄')); // emoji 也在符号引擎内
    expect(api.searchSymbols(keyword: 'xin'), isNotEmpty); // ♥ 的拼音关键词
  });

  test('custom dict load: nonexistent path falls back to builtin dict', () async {
    // engine_data::load_or_fallback 对不存在的路径回退内置词库（35 词），不报错。
    // 语义：加载成功 + 回退词库可用。真实报错路径只有词库文件损坏等无法覆盖的输入。
    final api = await Api.load(path: '/nonexistent/xx.opid');

    expect(api.buffer(), '');
    api.inputKey(ch: 'w');
    api.inputKey(ch: 'o');
    expect(api.candidates(limit: BigInt.from(8)).first.text, '我');
  });
}
