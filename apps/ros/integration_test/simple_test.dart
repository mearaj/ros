import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:ros/app.dart';
import 'package:ros/src/rust/api/simple.dart';
import 'package:ros/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets('connects Flutter to the Rust operational core', (tester) async {
    await tester.pumpWidget(
      RestaurantOperatingSystemApp(coreStatus: localCoreStatus()),
    );

    expect(find.textContaining('Rust operational core'), findsOneWidget);
  });
}
