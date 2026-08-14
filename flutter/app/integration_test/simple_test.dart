import 'package:flutter_test/flutter_test.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/main.dart';
import 'package:app/settings/settings_page.dart';
import 'package:app/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => await RustLib.init());
  testWidgets('Can call rust function', (WidgetTester tester) async {
    final controller = await EngineController.load();
    await tester.pumpWidget(MyApp(controller: controller));
    expect(find.byType(SettingsPage), findsOneWidget);
  });
}
