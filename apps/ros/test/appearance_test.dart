import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ros/app.dart';
import 'package:ros/src/rust/api/simple.dart';
import 'package:ros/theme/appearance.dart';

void main() {
  test('AppearanceStore round-trips theme modes', () async {
    final dir = await Directory.systemTemp.createTemp('ros-appearance-');
    addTearDown(() => dir.delete(recursive: true));

    expect(await AppearanceStore.load(dir.path), ThemeMode.light);

    await AppearanceStore.save(dir.path, ThemeMode.dark);
    expect(await AppearanceStore.load(dir.path), ThemeMode.dark);

    await AppearanceStore.save(dir.path, ThemeMode.system);
    expect(await AppearanceStore.load(dir.path), ThemeMode.system);

    await AppearanceStore.save(dir.path, ThemeMode.light);
    expect(await AppearanceStore.load(dir.path), ThemeMode.light);
  });

  testWidgets('appearance menu switches MaterialApp to dark theme', (
    tester,
  ) async {
    await tester.pumpWidget(
      const RestaurantOperatingSystemApp(
        coreStatus: 'Restaurant Operating System • Rust operational core',
        workspace: CommunityWorkspace(
          storageStatus: 'Saved locally • encrypted database ready',
          setupRequired: false,
          categories: [],
          products: [],
          customers: [],
          openDrafts: [],
          kitchenTickets: [],
        ),
        staffSecurity: CommunityStaffSecurity(
          storageStatus: 'Staff security needs attention',
          available: false,
          ownerPinSetupRequired: false,
          staff: [],
        ),
      ),
    );

    expect(
      tester.widget<MaterialApp>(find.byType(MaterialApp)).themeMode,
      ThemeMode.light,
    );
    expect(find.byKey(const Key('appearance-menu')), findsOneWidget);

    await tester.tap(find.byKey(const Key('appearance-menu')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Dark').last);
    await tester.pumpAndSettle();

    expect(
      tester.widget<MaterialApp>(find.byType(MaterialApp)).themeMode,
      ThemeMode.dark,
    );
  });
}
