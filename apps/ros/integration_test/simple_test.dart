import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:ros/app.dart';
import 'package:ros/src/rust/api/simple.dart';
import 'package:ros/src/rust/frb_generated.dart';

const _activeOwnerSecurity = CommunityStaffSecurity(
  storageStatus: 'Unlocked as Owner',
  available: true,
  ownerPinSetupRequired: false,
  activeStaffId: '0197d918-7e11-7000-8000-000000000003',
  activeStaffName: 'Owner',
  activeStaffRole: 'owner',
  staff: [
    CommunityStaffView(
      staffId: '0197d918-7e11-7000-8000-000000000003',
      displayName: 'Owner',
      role: 'owner',
      active: true,
      pinConfigured: true,
    ),
  ],
);

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets('connects Flutter to the Rust operational core', (tester) async {
    await tester.pumpWidget(
      RestaurantOperatingSystemApp(
        coreStatus: localCoreStatus(),
        staffSecurity: _activeOwnerSecurity,
      ),
    );

    expect(find.textContaining('Rust operational core'), findsOneWidget);
  });
}
