import 'package:app/src/rust/api/simple.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('greeting round trip', () async {
    await RustLib.init();
    expect(greet(name: 'world'), 'Hello, world!');
    expect(greet(name: '世界'), 'Hello, 世界!');
  });
}
