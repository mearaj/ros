import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ros/app.dart';
import 'package:ros/src/rust/api/simple.dart';

const _ownerStaffId = '0197d918-7e11-7000-8000-000000000003';
const _kitchenStaffId = '0197d918-7e11-7000-8000-000000000006';

const _activeOwnerSecurity = CommunityStaffSecurity(
  storageStatus: 'Unlocked as Owner',
  available: true,
  ownerPinSetupRequired: false,
  activeStaffId: _ownerStaffId,
  activeStaffName: 'Owner',
  activeStaffRole: 'owner',
  staff: [
    CommunityStaffView(
      staffId: _ownerStaffId,
      displayName: 'Owner',
      role: 'owner',
      active: true,
      pinConfigured: true,
    ),
  ],
);

const _activeKitchenSecurity = CommunityStaffSecurity(
  storageStatus: 'Unlocked as Kitchen',
  available: true,
  ownerPinSetupRequired: false,
  activeStaffId: _kitchenStaffId,
  activeStaffName: 'Kitchen',
  activeStaffRole: 'kitchen',
  staff: [
    CommunityStaffView(
      staffId: _kitchenStaffId,
      displayName: 'Kitchen',
      role: 'kitchen',
      active: true,
      pinConfigured: true,
    ),
  ],
);

void main() {
  testWidgets(
    'fails closed when provisioned data has no verified staff-security state',
    (tester) async {
      await tester.pumpWidget(
        const RestaurantOperatingSystemApp(
          coreStatus: 'Restaurant Operating System • Rust operational core',
          workspace: CommunityWorkspace(
            storageStatus: 'Saved locally • encrypted database ready',
            setupRequired: false,
            categories: [],
            products: [],
            customers: [
              CommunityCustomerView(
                customerId: '0197d918-7e11-7000-8000-000000000009',
                displayName: 'Sensitive customer',
                marketingConsent: false,
                revision: 1,
              ),
            ],
            openDrafts: [],
            kitchenTickets: [],
          ),
        ),
      );

      expect(find.text('Staff security needs attention'), findsOne);
      expect(find.text('Sensitive customer'), findsNothing);
      expect(find.text('New order'), findsNothing);
    },
  );

  testWidgets('keeps kitchen staff out of financial reporting navigation', (
    tester,
  ) async {
    await tester.pumpWidget(
      const RestaurantOperatingSystemApp(
        coreStatus: 'Restaurant Operating System • Rust operational core',
        staffSecurity: _activeKitchenSecurity,
      ),
    );

    await tester.tap(find.text('Reports'));
    await tester.pumpAndSettle();

    expect(
      find.text('This workspace is not available to the active role.'),
      findsOne,
    );
    expect(find.text('Your restaurant is ready for service.'), findsOne);
  });

  testWidgets(
    'keeps overview metric cards within a narrow desktop viewport at large text',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(940, 640));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);

      await tester.pumpWidget(
        const RestaurantOperatingSystemApp(
          coreStatus: 'Restaurant Operating System • Rust operational core',
          staffSecurity: _activeOwnerSecurity,
        ),
      );
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'uses the compact counter in a 960 px desktop shell after navigation takes its width',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(960, 720));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(
        const RestaurantOperatingSystemApp(
          coreStatus: 'Restaurant Operating System • Rust operational core',
          workspace: CommunityWorkspace(
            storageStatus: 'Saved locally • encrypted database ready',
            setupRequired: false,
            categories: [],
            customers: [],
            openDrafts: [],
            kitchenTickets: [],
            products: [
              CommunityProductView(
                productId: '0197d918-7e11-7000-8000-000000000002',
                categoryId: null,
                displayName: 'Masala chai',
                unitPriceMinor: 2500,
                currencyCode: 'INR',
                revision: 1,
                isAvailable: true,
                taxTreatment: 'no_tax',
                modifierOptions: [],
              ),
            ],
          ),
          staffSecurity: _activeOwnerSecurity,
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byType(NavigationRail), findsOneWidget);
      await tester.tap(find.text('New order'));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('pos-view-order')), findsOneWidget);
      expect(
        find.byKey(const PageStorageKey('pos-desktop-order-scroll')),
        findsNothing,
      );
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('renders the local-first service overview', (tester) async {
    await tester.pumpWidget(
      const RestaurantOperatingSystemApp(
        coreStatus: 'Restaurant Operating System • Rust operational core',
        staffSecurity: _activeOwnerSecurity,
      ),
    );

    expect(find.text('Your restaurant is ready for service.'), findsOneWidget);
    expect(find.text('Every order is saved locally first.'), findsOneWidget);
    expect(find.text('New order'), findsOneWidget);
  });

  testWidgets(
    'blocks normal kitchen progression after a cancellation request',
    (tester) async {
      await tester.pumpWidget(
        const RestaurantOperatingSystemApp(
          coreStatus: 'Restaurant Operating System • Rust operational core',
          workspace: CommunityWorkspace(
            storageStatus: 'Kitchen cancellation sent locally',
            setupRequired: false,
            categories: [],
            products: [],
            customers: [],
            openDrafts: [],
            kitchenTickets: [
              CommunityKitchenTicketView(
                ticketId: '0197d918-7e11-7000-8000-000000000004',
                tableLabel: 'Table 4',
                state: 'new',
                revision: 1,
                cancellationPending: true,
                kitchenNote: null,
                lines: [
                  CommunityKitchenTicketLineView(
                    displayName: 'Masala chai',
                    modifierNames: [],
                    quantity: 1,
                  ),
                ],
              ),
            ],
          ),
          staffSecurity: _activeKitchenSecurity,
        ),
      );

      await tester.tap(find.text('Kitchen'));
      await tester.pumpAndSettle();

      expect(
        find.textContaining('Cancellation requested — stop work'),
        findsOneWidget,
      );
      expect(find.text('Acknowledge cancellation'), findsOneWidget);
      expect(find.text('Start preparing'), findsNothing);
    },
  );

  testWidgets('shows an immutable kitchen instruction on Kitchen Display', (
    tester,
  ) async {
    await tester.pumpWidget(
      const RestaurantOperatingSystemApp(
        coreStatus: 'Restaurant Operating System • Rust operational core',
        workspace: CommunityWorkspace(
          storageStatus: 'Kitchen ticket saved locally',
          setupRequired: false,
          categories: [],
          products: [],
          customers: [],
          openDrafts: [],
          kitchenTickets: [
            CommunityKitchenTicketView(
              ticketId: '0197d918-7e11-7000-8000-000000000005',
              tableLabel: 'Table 8',
              state: 'new',
              revision: 1,
              cancellationPending: false,
              kitchenNote: 'No onions, please',
              lines: [
                CommunityKitchenTicketLineView(
                  displayName: 'Masala chai',
                  modifierNames: [],
                  quantity: 1,
                ),
              ],
            ),
          ],
        ),
        staffSecurity: _activeKitchenSecurity,
      ),
    );

    await tester.tap(find.text('Kitchen'));
    await tester.pumpAndSettle();

    expect(find.text('Kitchen instruction'), findsOneWidget);
    expect(find.text('No onions, please'), findsOneWidget);
  });

  testWidgets(
    'offers Community setup when a local workspace is unprovisioned',
    (tester) async {
      await tester.pumpWidget(
        const RestaurantOperatingSystemApp(
          coreStatus: 'Restaurant Operating System • Rust operational core',
          workspace: CommunityWorkspace(
            storageStatus:
                'Encrypted local storage ready • restaurant setup required',
            setupRequired: true,
            categories: [],
            products: [],
            customers: [],
            openDrafts: [],
            kitchenTickets: [],
          ),
          applicationSupportDirectory: '/test-support',
        ),
      );

      await tester.tap(find.text('Menu'));
      await tester.pumpAndSettle();

      expect(
        find.text('Create your local restaurant workspace'),
        findsOneWidget,
      );
      expect(find.text('Create local workspace'), findsOneWidget);
    },
  );

  testWidgets('blocks setup when secure local storage is unavailable', (
    tester,
  ) async {
    await tester.pumpWidget(
      const RestaurantOperatingSystemApp(
        coreStatus: 'Restaurant Operating System • Rust operational core',
        workspace: CommunityWorkspace(
          storageStatus:
              'Local storage needs attention • secure local storage is unavailable',
          setupRequired: true,
          categories: [],
          products: [],
          customers: [],
          openDrafts: [],
          kitchenTickets: [],
        ),
        applicationSupportDirectory: '/test-support',
      ),
    );

    await tester.tap(find.text('Menu'));
    await tester.pumpAndSettle();

    expect(find.text('Local storage needs attention'), findsOneWidget);
    expect(find.text('Retry storage'), findsOneWidget);
    expect(find.text('Create local workspace'), findsNothing);
  });

  testWidgets(
    'offers fresh setup when local data recovery is required before setup',
    (tester) async {
      await tester.pumpWidget(
        const RestaurantOperatingSystemApp(
          coreStatus: 'Restaurant Operating System • Rust operational core',
          workspace: CommunityWorkspace(
            storageStatus:
                'Local storage needs attention • local data recovery is required before setup',
            setupRequired: true,
            categories: [],
            products: [],
            customers: [],
            openDrafts: [],
            kitchenTickets: [],
          ),
          applicationSupportDirectory: '/test-support',
        ),
      );

      await tester.tap(find.byIcon(Icons.menu_book_outlined));
      await tester.pumpAndSettle();

      expect(find.text('Local storage needs attention'), findsOneWidget);
      expect(find.text('Start fresh setup'), findsOneWidget);
      expect(find.text('Retry storage'), findsOneWidget);
      expect(find.text('Create local workspace'), findsNothing);

      await tester.tap(find.text('POS'));
      await tester.pumpAndSettle();
      expect(
        find.text(
          'Secure local storage must be resolved before opening this workspace.',
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets('requires an owner PIN before a provisioned counter is exposed', (
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
        applicationSupportDirectory: '/test-support',
        staffSecurity: CommunityStaffSecurity(
          storageStatus:
              'Owner PIN setup required before restaurant operations',
          available: true,
          ownerPinSetupRequired: true,
          staff: [
            CommunityStaffView(
              staffId: '0197d918-7e11-7000-8000-000000000003',
              displayName: 'Owner',
              role: 'owner',
              active: true,
              pinConfigured: false,
            ),
          ],
        ),
      ),
    );

    expect(find.text('Secure your restaurant'), findsOneWidget);
    expect(find.byKey(const Key('owner-pin')), findsOneWidget);
    expect(find.text('New order'), findsNothing);

    await tester.enterText(find.byKey(const Key('owner-pin')), '12345');
    await tester.enterText(find.byKey(const Key('owner-pin-confirm')), '12345');
    await tester.tap(find.byKey(const Key('configure-owner-pin')));
    await tester.pump();
    expect(find.text('Use 6 to 12 digits'), findsOneWidget);
  });

  testWidgets('renders persisted Community categories in the menu workspace', (
    tester,
  ) async {
    await tester.pumpWidget(
      const RestaurantOperatingSystemApp(
        coreStatus: 'Restaurant Operating System • Rust operational core',
        workspace: CommunityWorkspace(
          storageStatus: 'Category saved locally',
          setupRequired: false,
          branchName: 'Koramangala',
          categories: [
            CommunityCategoryView(
              categoryId: '0197d918-7e11-7000-8000-000000000001',
              displayName: 'Hot drinks',
              revision: 1,
            ),
          ],
          customers: [],
          openDrafts: [],
          kitchenTickets: [],
          products: [
            CommunityProductView(
              productId: '0197d918-7e11-7000-8000-000000000002',
              categoryId: '0197d918-7e11-7000-8000-000000000001',
              displayName: 'Masala chai',
              unitPriceMinor: 2_500,
              currencyCode: 'INR',
              revision: 1,
              isAvailable: true,
              taxTreatment: 'no_tax',
              modifierOptions: [],
            ),
          ],
        ),
        staffSecurity: _activeOwnerSecurity,
      ),
    );

    await tester.tap(find.text('Menu'));
    await tester.pumpAndSettle();

    expect(find.text('Menu & products'), findsOneWidget);
    expect(find.text('Hot drinks'), findsOneWidget);
    expect(find.text('Category saved locally'), findsOneWidget);
    await tester.drag(
      find.byKey(const PageStorageKey('category-manager-scroll')),
      const Offset(0, -1200),
    );
    await tester.pumpAndSettle();
    expect(find.text('Masala chai'), findsOneWidget);
    expect(find.text('INR 25.00'), findsOneWidget);
  });

  testWidgets(
    'offers distinct app artwork, catalogue search, upload, and removal for a category image',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1024, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        const RestaurantOperatingSystemApp(
          coreStatus: 'Restaurant Operating System • Rust operational core',
          workspace: CommunityWorkspace(
            storageStatus: 'Category saved locally',
            setupRequired: false,
            categories: [
              CommunityCategoryView(
                categoryId: '0197d918-7e11-7000-8000-000000000001',
                displayName: 'Hot drinks',
                revision: 1,
                imageAssetKey: 'category_beverages',
              ),
            ],
            products: [],
            customers: [],
            openDrafts: [],
            kitchenTickets: [],
          ),
          staffSecurity: _activeOwnerSecurity,
          applicationSupportDirectory: '/test-support',
        ),
      );

      await tester.tap(find.byIcon(Icons.menu_book_outlined));
      await tester.pumpAndSettle();
      final manageImage = find.byTooltip('Manage category image');
      await tester.tap(manageImage);
      await tester.pumpAndSettle();

      expect(find.text('Choose app category artwork'), findsOneWidget);
      expect(find.text('Search Gotigin photos'), findsAtLeastNWidgets(1));
      expect(find.text('Use my restaurant image'), findsOneWidget);
      expect(find.text('Remove current image'), findsOneWidget);

      await tester.tap(find.text('Choose app category artwork'));
      await tester.pumpAndSettle();
      expect(find.text('Choose category artwork'), findsOneWidget);
      expect(find.text('Beverages'), findsOneWidget);
      expect(find.text('Rice & bowls'), findsOneWidget);
    },
  );

  testWidgets('requires a reason before archiving a menu item', (tester) async {
    await tester.pumpWidget(
      const RestaurantOperatingSystemApp(
        coreStatus: 'Restaurant Operating System • Rust operational core',
        workspace: CommunityWorkspace(
          storageStatus: 'Saved locally • encrypted database ready',
          setupRequired: false,
          categories: [
            CommunityCategoryView(
              categoryId: '0197d918-7e11-7000-8000-000000000001',
              displayName: 'Hot drinks',
              revision: 1,
            ),
          ],
          customers: [],
          openDrafts: [],
          kitchenTickets: [],
          products: [
            CommunityProductView(
              productId: '0197d918-7e11-7000-8000-000000000002',
              categoryId: '0197d918-7e11-7000-8000-000000000001',
              displayName: 'Masala chai',
              unitPriceMinor: 2_500,
              currencyCode: 'INR',
              revision: 1,
              isAvailable: true,
              taxTreatment: 'no_tax',
              modifierOptions: [],
            ),
          ],
        ),
        staffSecurity: _activeOwnerSecurity,
      ),
    );

    await tester.tap(find.text('Menu'));
    await tester.pumpAndSettle();
    await tester.drag(
      find.byKey(const PageStorageKey('category-manager-scroll')),
      const Offset(0, -1200),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Manage Masala chai'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Remove from active menu'));
    await tester.pumpAndSettle();

    expect(find.text('Remove from active menu?'), findsOneWidget);
    expect(find.text('Archive item'), findsOneWidget);
    expect(
      tester
          .widget<FilledButton>(
            find.byKey(const ValueKey('archive-product-confirm')),
          )
          .onPressed,
      isNull,
    );

    await tester.enterText(
      find.byKey(const ValueKey('archive-product-reason')),
      'No longer offered',
    );
    await tester.pump();
    expect(
      tester
          .widget<FilledButton>(
            find.byKey(const ValueKey('archive-product-confirm')),
          )
          .onPressed,
      isNotNull,
    );
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
  });
}
