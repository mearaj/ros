import 'dart:ui' show SemanticsAction, Tristate;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:ros/features/point_of_sale/pos_workspace.dart';
import 'package:ros/src/rust/api/simple.dart';
import 'package:ros/theme/app_theme.dart';

const _productId = '0197d918-7e11-7000-8000-000000000002';
const _extraCheeseId = '0197d918-7e11-7000-8000-000000000008';

void main() {
  testWidgets(
    'exposes an available menu item as one descriptive semantic add action',
    (tester) async {
      final semantics = tester.ensureSemantics();
      try {
        await tester.pumpWidget(
          _CounterHarness(
            onCheckout: (request) async =>
                const PosCheckoutResult(recorded: false, status: 'Unused'),
          ),
        );
        await tester.pumpAndSettle();

        final addAction = find.bySemanticsLabel('Add Masala chai, INR 25.00');
        expect(addAction, findsOneWidget);

        final semanticsData = tester.getSemantics(addAction).getSemanticsData();
        expect(semanticsData.flagsCollection.isButton, isTrue);
        expect(semanticsData.flagsCollection.isEnabled, Tristate.isTrue);
        expect(semanticsData.hasAction(SemanticsAction.tap), isTrue);
        expect(semanticsData.label, 'Add Masala chai, INR 25.00');
        expect(semanticsData.hint, 'Adds one item to the current order.');
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets('supports keyboard search and product activation', (
    tester,
  ) async {
    await tester.pumpWidget(
      _CounterHarness(
        onCheckout: (request) async =>
            const PosCheckoutResult(recorded: false, status: 'Unused'),
      ),
    );
    await tester.pumpAndSettle();

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyF);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();

    final search = tester.widget<EditableText>(find.byType(EditableText));
    expect(search.focusNode.hasFocus, isTrue);

    // Search -> All items -> Hot drinks -> first product card.
    for (var index = 0; index < 3; index++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();

    expect(find.text('Current order • 1 item'), findsOneWidget);
  });

  testWidgets('keeps every category tab reachable by horizontal scrolling', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(720, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    const categories = <CommunityCategoryView>[
      CommunityCategoryView(
        categoryId: 'c-beverages',
        displayName: 'Beverages',
        revision: 1,
      ),
      CommunityCategoryView(
        categoryId: 'c-breakfast',
        displayName: 'Breakfast',
        revision: 1,
      ),
      CommunityCategoryView(
        categoryId: 'c-starters',
        displayName: 'Snacks & Starters',
        revision: 1,
      ),
      CommunityCategoryView(
        categoryId: 'c-mains',
        displayName: 'Main Course',
        revision: 1,
      ),
      CommunityCategoryView(
        categoryId: 'c-breads',
        displayName: 'Breads',
        revision: 1,
      ),
      CommunityCategoryView(
        categoryId: 'c-rice',
        displayName: 'Rice & Biryani',
        revision: 1,
      ),
      CommunityCategoryView(
        categoryId: 'c-desserts',
        displayName: 'Desserts',
        revision: 1,
      ),
    ];

    await tester.pumpWidget(
      _CounterHarness(
        workspace: CommunityWorkspace(
          storageStatus: 'Saved locally • encrypted database ready',
          setupRequired: false,
          branchName: 'Koramangala',
          categories: categories,
          customers: const [],
          openDrafts: const [],
          kitchenTickets: const [],
          products: [
            for (final category in categories)
              CommunityProductView(
                productId: 'p-${category.categoryId}',
                categoryId: category.categoryId,
                displayName: '${category.displayName} item',
                unitPriceMinor: 1000,
                currencyCode: 'INR',
                revision: 1,
                isAvailable: true,
                taxTreatment: 'no_tax',
                modifierOptions: const [],
              ),
          ],
        ),
        onCheckout: (request) async =>
            const PosCheckoutResult(recorded: false, status: 'Unused'),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('All items'), findsOneWidget);
    expect(find.text('Beverages'), findsOneWidget);
    expect(find.byType(TabBar), findsOneWidget);
    expect(find.byKey(const Key('pos-category-scroll-left')), findsOneWidget);
    expect(find.byKey(const Key('pos-category-scroll-right')), findsOneWidget);

    await tester.tap(find.byKey(const Key('pos-category-scroll-right')));
    await tester.pumpAndSettle();
    expect(find.text('Beverages item'), findsOneWidget);

    final desserts = find.text('Desserts');
    await tester.ensureVisible(desserts);
    await tester.pumpAndSettle();
    expect(desserts, findsOneWidget);
    await tester.tap(desserts);
    await tester.pumpAndSettle();
    expect(find.text('Desserts item'), findsOneWidget);
  });

  testWidgets(
    'keeps critical counter controls large and announces cart quantities',
    (tester) async {
      final semantics = tester.ensureSemantics();
      try {
        await tester.pumpWidget(
          _CounterHarness(
            onCheckout: (request) async =>
                const PosCheckoutResult(recorded: false, status: 'Unused'),
          ),
        );
        await tester.pumpAndSettle();

        final productAction = find.byKey(const Key('pos-add-$_productId'));
        final productSize = tester.getSize(productAction);
        expect(productSize.width, greaterThanOrEqualTo(48));
        expect(productSize.height, greaterThanOrEqualTo(48));
        await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
        await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));

        await tester.tap(productAction);
        await tester.tap(find.byKey(const Key('pos-view-order')));
        await tester.pumpAndSettle();

        expect(
          find.bySemanticsLabel(
            'Masala chai, quantity 1, line total INR 25.00',
          ),
          findsOneWidget,
        );
        for (final key in const [
          Key('pos-remove-one-$_productId'),
          Key('pos-add-one-$_productId'),
        ]) {
          final size = tester.getSize(find.byKey(key));
          expect(size.width, greaterThanOrEqualTo(48));
          expect(size.height, greaterThanOrEqualTo(48));
        }
        await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
        await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets('remains usable at 200 percent text scaling', (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 640));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      _CounterHarness(
        textScaler: const TextScaler.linear(2),
        onCheckout: (request) async =>
            const PosCheckoutResult(recorded: false, status: 'Unused'),
      ),
    );
    await tester.pumpAndSettle();

    final productAction = find.byKey(const Key('pos-add-$_productId'));
    await tester.drag(
      find.byKey(const PageStorageKey('pos-catalog-scroll')),
      const Offset(0, -400),
    );
    await tester.pumpAndSettle();
    await tester.tap(productAction);
    await tester.tap(find.byKey(const Key('pos-view-order')));
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    final orderScroll = find.descendant(
      of: find.byKey(const PageStorageKey('pos-order-scroll')),
      matching: find.byType(Scrollable),
    );
    await tester.scrollUntilVisible(
      find.byKey(const Key('pos-checkout')),
      300,
      scrollable: orderScroll,
    );
    expect(find.byKey(const Key('pos-checkout')), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'records only trusted cart selections and clears a cart after local success',
    (tester) async {
      PosCheckoutRequest? receivedRequest;
      await tester.pumpWidget(
        _CounterHarness(
          onCheckout: (request) async {
            receivedRequest = request;
            return const PosCheckoutResult(
              recorded: true,
              status: 'Invoice INV-000001 saved locally.',
              invoiceNumber: 'INV-000001',
              totalMinor: 5_000,
              currencyCode: 'INR',
              paymentMethod: 'cash',
            );
          },
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('pos-add-$_productId')));
      await tester.tap(find.byKey(const Key('pos-add-$_productId')));
      await tester.tap(find.byKey(const Key('pos-view-order')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Dine in'));
      await tester.pump();

      expect(find.byKey(const Key('pos-cart-$_productId')), findsOneWidget);

      await tester.drag(
        find.byKey(const PageStorageKey('pos-order-scroll')),
        const Offset(0, -500),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pos-checkout')));
      await tester.pumpAndSettle();

      expect(receivedRequest, isNotNull);
      expect(receivedRequest!.fulfillment, 'dine_in');
      expect(receivedRequest!.paymentMethod, 'cash');
      expect(receivedRequest!.lines, const <PosCartLine>[
        PosCartLine(productId: _productId, quantity: 2),
      ]);
      expect(find.text('Current order is empty'), findsOneWidget);
    },
  );

  testWidgets(
    'keeps modifier identities in the POS cart while Rust remains price authority',
    (tester) async {
      PosCheckoutRequest? receivedRequest;
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          home: Scaffold(
            body: PosWorkspace(
              workspace: const CommunityWorkspace(
                storageStatus: 'Saved locally • encrypted database ready',
                setupRequired: false,
                categories: <CommunityCategoryView>[],
                customers: <CommunityCustomerView>[],
                openDrafts: <CommunityDraftOrderView>[],
                kitchenTickets: <CommunityKitchenTicketView>[],
                products: <CommunityProductView>[
                  CommunityProductView(
                    productId: _productId,
                    categoryId: null,
                    displayName: 'Masala chai',
                    unitPriceMinor: 2_500,
                    currencyCode: 'INR',
                    revision: 1,
                    isAvailable: true,
                    taxTreatment: 'no_tax',
                    modifierOptions: <CommunityModifierOptionView>[
                      CommunityModifierOptionView(
                        modifierOptionId: _extraCheeseId,
                        displayName: 'Extra cheese',
                        priceDeltaMinor: 500,
                        currencyCode: 'INR',
                        revision: 1,
                        archived: false,
                      ),
                    ],
                  ),
                ],
              ),
              isSaving: false,
              onPreviewPricing: _stubPreviewPricing,
              onCheckout: (request) async {
                receivedRequest = request;
                return const PosCheckoutResult(
                  recorded: false,
                  status: 'Checked by Rust in production',
                );
              },
              onSaveDraft: (request) async =>
                  const PosDraftResult(saved: false, status: 'Unused'),
              onSendToKitchen: (draftOrderId, revision) async =>
                  const PosDraftActionResult(
                    succeeded: false,
                    status: 'Unused',
                  ),
              onOpenMenu: () {},
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('pos-add-$_productId')));
      await tester.pumpAndSettle();
      expect(find.text('Customise Masala chai'), findsOneWidget);
      await tester.tap(find.text('Extra cheese'));
      await tester.tap(find.text('Add to order'));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('pos-view-order')));
      await tester.pumpAndSettle();
      expect(find.text('Extra cheese'), findsOneWidget);
      expect(
        find.byKey(const Key('pos-cart-$_productId:$_extraCheeseId')),
        findsOneWidget,
      );
      await tester.drag(
        find.byKey(const PageStorageKey('pos-order-scroll')),
        const Offset(0, -500),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pos-checkout')));
      await tester.pumpAndSettle();

      expect(receivedRequest?.lines, const <PosCartLine>[
        PosCartLine(
          productId: _productId,
          quantity: 1,
          modifierOptionIds: <String>[_extraCheeseId],
        ),
      ]);
    },
  );

  testWidgets('keeps the cart intact when local persistence declines a sale', (
    tester,
  ) async {
    await tester.pumpWidget(
      _CounterHarness(
        onCheckout: (request) async => const PosCheckoutResult(
          recorded: false,
          status:
              'Sale needs attention • the local sale could not be recorded. Your cart is still here.',
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('pos-add-$_productId')));
    await tester.tap(find.byKey(const Key('pos-view-order')));
    await tester.pumpAndSettle();
    await tester.drag(
      find.byKey(const PageStorageKey('pos-order-scroll')),
      const Offset(0, -500),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('pos-checkout')));
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('pos-cart-$_productId')), findsOneWidget);
    expect(find.textContaining('Your cart is still here.'), findsOneWidget);
  });

  testWidgets(
    'recovers from a hold failure without losing or locking the cart',
    (tester) async {
      await tester.pumpWidget(
        _CounterHarness(
          onCheckout: (request) async =>
              const PosCheckoutResult(recorded: false, status: 'Unused'),
          onSaveDraft: (request) async => throw StateError('simulated failure'),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('pos-add-$_productId')));
      await tester.tap(find.byKey(const Key('pos-view-order')));
      await tester.pumpAndSettle();
      await tester.drag(
        find.byKey(const PageStorageKey('pos-order-scroll')),
        const Offset(0, -500),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pos-save-draft')));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byKey(const Key('pos-cart-$_productId')), findsOneWidget);
      expect(find.textContaining('Your cart is still here.'), findsOneWidget);
      expect(
        tester
            .widget<FilledButton>(find.byKey(const Key('pos-save-draft')))
            .onPressed,
        isNotNull,
      );
    },
  );

  testWidgets('recovers from a kitchen send exception with the draft intact', (
    tester,
  ) async {
    await tester.pumpWidget(
      _CounterHarness(
        onCheckout: (request) async =>
            const PosCheckoutResult(recorded: false, status: 'Unused'),
        onSendToKitchen: (draftOrderId, revision) async =>
            throw StateError('simulated failure'),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('pos-add-$_productId')));
    await tester.tap(find.byKey(const Key('pos-view-order')));
    await tester.pumpAndSettle();
    await tester.drag(
      find.byKey(const PageStorageKey('pos-order-scroll')),
      const Offset(0, -500),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('pos-save-draft')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('pos-send-to-kitchen')));
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.byKey(const Key('pos-cart-$_productId')), findsOneWidget);
    expect(find.textContaining('It is still held locally.'), findsOneWidget);
    expect(
      tester
          .widget<FilledButton>(find.byKey(const Key('pos-send-to-kitchen')))
          .onPressed,
      isNotNull,
    );
  });

  testWidgets('keeps temporarily sold-out menu items out of checkout', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: PosWorkspace(
            workspace: const CommunityWorkspace(
              storageStatus: 'Saved locally • encrypted database ready',
              setupRequired: false,
              categories: <CommunityCategoryView>[],
              customers: <CommunityCustomerView>[],
              openDrafts: <CommunityDraftOrderView>[],
              kitchenTickets: <CommunityKitchenTicketView>[],
              products: <CommunityProductView>[
                CommunityProductView(
                  productId: _productId,
                  categoryId: null,
                  displayName: 'Masala chai',
                  unitPriceMinor: 2_500,
                  currencyCode: 'INR',
                  revision: 3,
                  isAvailable: false,
                  taxTreatment: 'no_tax',
                  modifierOptions: <CommunityModifierOptionView>[],
                ),
              ],
            ),
            isSaving: false,
            onPreviewPricing: _stubPreviewPricing,
            onCheckout: (request) async =>
                const PosCheckoutResult(recorded: false, status: 'Unused'),
            onSaveDraft: (request) async =>
                const PosDraftResult(saved: false, status: 'Unused'),
            onSendToKitchen: (draftOrderId, revision) async =>
                const PosDraftActionResult(succeeded: false, status: 'Unused'),
            onOpenMenu: () {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('pos-add-$_productId')), findsNothing);
    expect(find.text('No items are ready to sell yet'), findsOneWidget);
    expect(find.textContaining('Resume selling'), findsOneWidget);
  });

  testWidgets('sends exact split-tender allocations to the trusted checkout', (
    tester,
  ) async {
    PosCheckoutRequest? receivedRequest;
    await tester.pumpWidget(
      _CounterHarness(
        onCheckout: (request) async {
          receivedRequest = request;
          return const PosCheckoutResult(recorded: true, status: 'Saved');
        },
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('pos-add-$_productId')));
    await tester.tap(find.byKey(const Key('pos-view-order')));
    await tester.pumpAndSettle();
    await tester.drag(
      find.byKey(const PageStorageKey('pos-order-scroll')),
      const Offset(0, -500),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Split'));
    await tester.tap(find.byKey(const Key('pos-checkout')));
    await tester.pumpAndSettle();
    expect(find.text('Split payment'), findsOneWidget);
    final inputs = find.byType(TextField);
    await tester.enterText(inputs.at(0), '10.00');
    await tester.enterText(inputs.at(2), '15.00');
    await tester.tap(find.text('Use split payment'));
    await tester.pumpAndSettle();

    expect(receivedRequest, isNotNull);
    expect(receivedRequest!.paymentMethod, 'cash');
    expect(receivedRequest!.paymentAllocations, const [
      CommunityPaymentAllocation(paymentMethod: 'cash', amountMinor: 1000),
      CommunityPaymentAllocation(paymentMethod: 'upi', amountMinor: 1500),
    ]);
  });

  testWidgets('requires an explicit table before holding a dine-in order', (
    tester,
  ) async {
    PosDraftRequest? receivedDraft;
    await tester.pumpWidget(
      _CounterHarness(
        onCheckout: (request) async =>
            const PosCheckoutResult(recorded: false, status: 'Unused'),
        onSaveDraft: (request) async {
          receivedDraft = request;
          return const PosDraftResult(
            saved: true,
            status: 'Open order saved locally',
            draftOrderId: '0197d918-7e11-7000-8000-000000000003',
            revision: 1,
          );
        },
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('pos-add-$_productId')));
    await tester.tap(find.byKey(const Key('pos-view-order')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Dine in'));
    await tester.drag(
      find.byKey(const PageStorageKey('pos-order-scroll')),
      const Offset(0, -500),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('pos-save-draft')));
    await tester.pumpAndSettle();
    expect(find.text('Choose table'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'Table 9');
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();

    expect(receivedDraft, isNotNull);
    expect(receivedDraft!.fulfillment, 'dine_in');
    expect(receivedDraft!.tableName, 'Table 9');
  });

  testWidgets(
    'saves an optional kitchen instruction with the retained draft revision',
    (tester) async {
      PosDraftRequest? receivedDraft;
      await tester.pumpWidget(
        _CounterHarness(
          onCheckout: (request) async =>
              const PosCheckoutResult(recorded: false, status: 'Unused'),
          onSaveDraft: (request) async {
            receivedDraft = request;
            return const PosDraftResult(
              saved: true,
              status: 'Open order saved locally',
              draftOrderId: _draftId,
              revision: 1,
            );
          },
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('pos-add-$_productId')));
      await tester.tap(find.byKey(const Key('pos-view-order')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pos-edit-kitchen-note')));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('pos-kitchen-note')),
        'No onions, please',
      );
      await tester.tap(find.byKey(const Key('pos-save-kitchen-note')));
      await tester.pumpAndSettle();

      await tester.drag(
        find.byKey(const PageStorageKey('pos-order-scroll')),
        const Offset(0, -500),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pos-save-draft')));
      await tester.pumpAndSettle();

      expect(receivedDraft, isNotNull);
      expect(receivedDraft!.kitchenNote, 'No onions, please');
    },
  );

  testWidgets(
    'saves a changed kitchen instruction before allowing a kitchen send',
    (tester) async {
      final savedDrafts = <PosDraftRequest>[];
      var sendCalls = 0;
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          home: Scaffold(
            body: PosWorkspace(
              workspace: const CommunityWorkspace(
                storageStatus: 'Saved locally • encrypted database ready',
                setupRequired: false,
                categories: <CommunityCategoryView>[
                  CommunityCategoryView(
                    categoryId: '0197d918-7e11-7000-8000-000000000001',
                    displayName: 'Hot drinks',
                    revision: 1,
                  ),
                ],
                products: <CommunityProductView>[
                  CommunityProductView(
                    productId: _productId,
                    categoryId: '0197d918-7e11-7000-8000-000000000001',
                    displayName: 'Masala chai',
                    unitPriceMinor: 2_500,
                    currencyCode: 'INR',
                    revision: 1,
                    isAvailable: true,
                    taxTreatment: 'no_tax',
                    modifierOptions: <CommunityModifierOptionView>[],
                  ),
                ],
                customers: <CommunityCustomerView>[],
                openDrafts: <CommunityDraftOrderView>[],
                kitchenTickets: <CommunityKitchenTicketView>[],
              ),
              isSaving: false,
              onPreviewPricing: _stubPreviewPricing,
              onCheckout: (request) async =>
                  const PosCheckoutResult(recorded: false, status: 'Unused'),
              onSaveDraft: (request) async {
                savedDrafts.add(request);
                return PosDraftResult(
                  saved: true,
                  status: 'Open order saved locally',
                  draftOrderId: _draftId,
                  revision: savedDrafts.length,
                );
              },
              onSendToKitchen: (draftOrderId, revision) async {
                sendCalls++;
                return const PosDraftActionResult(
                  succeeded: true,
                  status: 'Unexpected send',
                );
              },
              onOpenMenu: () {},
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('pos-add-$_productId')));
      await tester.tap(find.byKey(const Key('pos-view-order')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pos-edit-kitchen-note')));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('pos-kitchen-note')),
        'No onions',
      );
      await tester.tap(find.byKey(const Key('pos-save-kitchen-note')));
      await tester.pumpAndSettle();
      await tester.drag(
        find.byKey(const PageStorageKey('pos-order-scroll')),
        const Offset(0, -500),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pos-save-draft')));
      await tester.pumpAndSettle();

      await tester.drag(
        find.byKey(const PageStorageKey('pos-order-scroll')),
        const Offset(0, 500),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pos-edit-kitchen-note')));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('pos-kitchen-note')),
        'Less sugar',
      );
      await tester.tap(find.byKey(const Key('pos-save-kitchen-note')));
      await tester.pumpAndSettle();

      await tester.drag(
        find.byKey(const PageStorageKey('pos-order-scroll')),
        const Offset(0, -500),
      );
      await tester.pumpAndSettle();
      expect(find.text('Save changes before sending'), findsOneWidget);
      await tester.tap(find.byKey(const Key('pos-send-to-kitchen')));
      await tester.pumpAndSettle();

      expect(sendCalls, 0);
      expect(savedDrafts, hasLength(2));
      expect(savedDrafts.last.kitchenNote, 'Less sugar');
      expect(savedDrafts.last.draftOrderId, _draftId);
      expect(savedDrafts.last.expectedRevision, 1);
    },
  );

  testWidgets(
    'restores a sent order as locked while keeping its settlement identity',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1200, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      PosCheckoutRequest? receivedRequest;
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          home: Scaffold(
            body: PosWorkspace(
              workspace: _workspaceWithDraft('sent_to_kitchen'),
              isSaving: false,
              onPreviewPricing: _stubPreviewPricing,
              onCheckout: (request) async {
                receivedRequest = request;
                return const PosCheckoutResult(
                  recorded: false,
                  status: 'Payment needs attention • test result',
                );
              },
              onSaveDraft: (request) async =>
                  const PosDraftResult(saved: false, status: 'Unused'),
              onSendToKitchen: (draftOrderId, revision) async =>
                  const PosDraftActionResult(
                    succeeded: false,
                    status: 'Unexpected resend',
                  ),
              onOpenMenu: () {},
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('Open & sent orders (1)'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Table 4'));
      await tester.pumpAndSettle();

      expect(
        find.textContaining('Kitchen-sent items and service are locked'),
        findsOneWidget,
      );
      expect(find.byKey(const Key('pos-send-to-kitchen')), findsNothing);
      expect(
        tester
            .widget<FilledButton>(find.byKey(const Key('pos-save-draft')))
            .onPressed,
        isNull,
      );

      await tester.drag(
        find.byKey(const PageStorageKey('pos-desktop-order-scroll')),
        const Offset(0, -600),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('pos-checkout')));
      await tester.pumpAndSettle();
      expect(receivedRequest, isNotNull);
      expect(receivedRequest!.draftOrderId, _draftId);
      expect(receivedRequest!.expectedDraftRevision, 4);
      expect(receivedRequest!.fulfillment, 'dine_in');
      expect(receivedRequest!.lines, const <PosCartLine>[
        PosCartLine(productId: _productId, quantity: 1),
      ]);
    },
  );

  testWidgets(
    'keeps a short wide restored kitchen order panel free of layout overflow',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1200, 600));
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          home: Scaffold(
            body: PosWorkspace(
              workspace: _workspaceWithDraft('sent_to_kitchen'),
              isSaving: false,
              onPreviewPricing: _stubPreviewPricing,
              onCheckout: (request) async => const PosCheckoutResult(
                recorded: false,
                status: 'Payment needs attention • test result',
              ),
              onSaveDraft: (request) async =>
                  const PosDraftResult(saved: false, status: 'Unused'),
              onSendToKitchen: (draftOrderId, revision) async =>
                  const PosDraftActionResult(
                    succeeded: false,
                    status: 'Unexpected resend',
                  ),
              onOpenMenu: () {},
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('Open & sent orders (1)'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Table 4'));
      await tester.pumpAndSettle();

      expect(
        find.textContaining('Kitchen-sent items and service are locked'),
        findsOneWidget,
      );
      expect(
        find.byKey(const PageStorageKey('pos-desktop-order-scroll')),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('retains a saved draft identity when kitchen send fails', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1200, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    String? sentDraftId;
    int? sentRevision;
    PosCheckoutRequest? receivedRequest;
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: PosWorkspace(
            workspace: _workspaceWithDraft('open'),
            isSaving: false,
            onPreviewPricing: _stubPreviewPricing,
            onCheckout: (request) async {
              receivedRequest = request;
              return const PosCheckoutResult(
                recorded: false,
                status: 'Payment needs attention • test result',
              );
            },
            onSaveDraft: (request) async =>
                const PosDraftResult(saved: false, status: 'Unused'),
            onSendToKitchen: (draftOrderId, revision) async {
              sentDraftId = draftOrderId;
              sentRevision = revision;
              return const PosDraftActionResult(
                succeeded: false,
                status: 'Kitchen needs attention • simulated local failure',
              );
            },
            onOpenMenu: () {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Open & sent orders (1)'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Table 4'));
    await tester.pumpAndSettle();
    await tester.drag(
      find.byKey(const PageStorageKey('pos-desktop-order-scroll')),
      const Offset(0, -600),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('pos-send-to-kitchen')));
    await tester.pumpAndSettle();

    expect(sentDraftId, _draftId);
    expect(sentRevision, 4);
    expect(find.byKey(const Key('pos-send-to-kitchen')), findsOneWidget);

    await tester.drag(
      find.byKey(const PageStorageKey('pos-desktop-order-scroll')),
      const Offset(0, -600),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('pos-checkout')));
    await tester.pumpAndSettle();
    expect(receivedRequest, isNotNull);
    expect(receivedRequest!.draftOrderId, _draftId);
    expect(receivedRequest!.expectedDraftRevision, 4);
  });

  testWidgets(
    'does not expose a checkout path while secure local storage needs attention',
    (tester) async {
      var checkoutCalls = 0;
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          home: Scaffold(
            body: PosWorkspace(
              workspace: const CommunityWorkspace(
                storageStatus:
                    'Local storage needs attention • secure local storage is unavailable',
                setupRequired: false,
                categories: <CommunityCategoryView>[],
                customers: <CommunityCustomerView>[],
                openDrafts: <CommunityDraftOrderView>[],
                kitchenTickets: <CommunityKitchenTicketView>[],
                products: <CommunityProductView>[
                  CommunityProductView(
                    productId: _productId,
                    categoryId: null,
                    displayName: 'Masala chai',
                    unitPriceMinor: 2_500,
                    currencyCode: 'INR',
                    revision: 1,
                    isAvailable: true,
                    taxTreatment: 'no_tax',
                    modifierOptions: <CommunityModifierOptionView>[],
                  ),
                ],
              ),
              isSaving: false,
              onPreviewPricing: _stubPreviewPricing,
              onCheckout: (request) async {
                checkoutCalls += 1;
                return const PosCheckoutResult(
                  recorded: false,
                  status: 'Unexpected',
                );
              },
              onSaveDraft: (request) async =>
                  const PosDraftResult(saved: false, status: 'Unexpected'),
              onSendToKitchen: (draftOrderId, revision) async =>
                  const PosDraftActionResult(
                    succeeded: false,
                    status: 'Unexpected',
                  ),
              onOpenMenu: () {},
            ),
          ),
        ),
      );

      expect(
        find.text('Counter is locked until local storage is ready'),
        findsOneWidget,
      );
      expect(find.byKey(const Key('pos-checkout')), findsNothing);
      expect(checkoutCalls, 0);
    },
  );
}

Future<PosPricingPreview> _stubPreviewPricing({
  required List<PosCartLine> lines,
  int? discountFixedMinor,
  int? discountPercentageBasisPoints,
  int? discountPercentageCapMinor,
  String? discountReason,
}) async {
  var subtotal = 0;
  for (final line in lines) {
    var unit = 2_500;
    for (final _ in line.modifierOptionIds) {
      unit += 500;
    }
    subtotal += unit * line.quantity;
  }
  var discount = discountFixedMinor ?? 0;
  if (discountPercentageBasisPoints != null) {
    discount = ((subtotal * discountPercentageBasisPoints) / 10000).round();
    final cap = discountPercentageCapMinor;
    if (cap != null && discount > cap) {
      discount = cap;
    }
  }
  if (discount > subtotal) {
    discount = subtotal;
  }
  return PosPricingPreview(
    available: true,
    status: 'Preview ready',
    subtotalMinor: subtotal,
    discountMinor: discount,
    taxMinor: 0,
    payableMinor: subtotal - discount,
    currencyCode: 'INR',
  );
}

class _CounterHarness extends StatelessWidget {
  const _CounterHarness({
    required this.onCheckout,
    this.onSaveDraft,
    this.onSendToKitchen,
    this.textScaler,
    this.workspace = const CommunityWorkspace(
      storageStatus: 'Saved locally • encrypted database ready',
      setupRequired: false,
      branchName: 'Koramangala',
      categories: <CommunityCategoryView>[
        CommunityCategoryView(
          categoryId: '0197d918-7e11-7000-8000-000000000001',
          displayName: 'Hot drinks',
          revision: 1,
        ),
      ],
      customers: <CommunityCustomerView>[],
      openDrafts: <CommunityDraftOrderView>[],
      kitchenTickets: <CommunityKitchenTicketView>[],
      products: <CommunityProductView>[
        CommunityProductView(
          productId: _productId,
          categoryId: '0197d918-7e11-7000-8000-000000000001',
          displayName: 'Masala chai',
          unitPriceMinor: 2_500,
          currencyCode: 'INR',
          revision: 1,
          isAvailable: true,
          taxTreatment: 'no_tax',
          modifierOptions: <CommunityModifierOptionView>[],
        ),
      ],
    ),
  });

  final PersistPosCheckout onCheckout;
  final PersistPosDraft? onSaveDraft;
  final SendPosDraftToKitchen? onSendToKitchen;
  final TextScaler? textScaler;
  final CommunityWorkspace workspace;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      theme: AppTheme.light(),
      builder: textScaler == null
          ? null
          : (context, child) => MediaQuery(
              data: MediaQuery.of(context).copyWith(textScaler: textScaler),
              child: child!,
            ),
      home: Scaffold(
        body: PosWorkspace(
          workspace: workspace,
          isSaving: false,
          onPreviewPricing: _stubPreviewPricing,
          onCheckout: onCheckout,
          onSaveDraft:
              onSaveDraft ??
              (request) async => const PosDraftResult(
                saved: true,
                status: 'Open order saved locally',
                draftOrderId: '0197d918-7e11-7000-8000-000000000003',
                revision: 1,
              ),
          onSendToKitchen:
              onSendToKitchen ??
              (draftOrderId, revision) async => const PosDraftActionResult(
                succeeded: false,
                status: 'Unused',
              ),
          onOpenMenu: () {},
        ),
      ),
    );
  }
}

const _draftId = '0197d918-7e11-7000-8000-000000000003';

CommunityWorkspace _workspaceWithDraft(String draftState) => CommunityWorkspace(
  storageStatus: 'Saved locally • encrypted database ready',
  setupRequired: false,
  branchName: 'Koramangala',
  categories: const <CommunityCategoryView>[
    CommunityCategoryView(
      categoryId: '0197d918-7e11-7000-8000-000000000001',
      displayName: 'Hot drinks',
      revision: 1,
    ),
  ],
  customers: const <CommunityCustomerView>[],
  openDrafts: <CommunityDraftOrderView>[
    CommunityDraftOrderView(
      draftOrderId: _draftId,
      fulfillment: 'dine_in',
      draftState: draftState,
      tableName: 'Table 4',
      kitchenNote: null,
      revision: 4,
      subtotalMinor: 2_500,
      currencyCode: 'INR',
      lineCount: 1,
      lines: const <CommunityCartLine>[
        CommunityCartLine(
          productId: _productId,
          quantity: 1,
          modifierOptionIds: <String>[],
        ),
      ],
    ),
  ],
  kitchenTickets: const <CommunityKitchenTicketView>[],
  products: const <CommunityProductView>[
    CommunityProductView(
      productId: _productId,
      categoryId: '0197d918-7e11-7000-8000-000000000001',
      displayName: 'Masala chai',
      unitPriceMinor: 2_500,
      currencyCode: 'INR',
      revision: 1,
      isAvailable: true,
      taxTreatment: 'no_tax',
      modifierOptions: <CommunityModifierOptionView>[],
    ),
  ],
);
