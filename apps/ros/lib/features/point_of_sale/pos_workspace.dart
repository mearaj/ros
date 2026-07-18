import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../catalog/menu_item_image.dart';
import '../../features/command_center/money_input.dart';
import '../../src/rust/api/simple.dart';

const _maximumModifierOptionsPerSaleLine = 20;

/// A cart line deliberately contains no client-calculated price or total.
/// The Rust sale command reloads the active product price inside its local
/// transaction before it records a sale.
@immutable
class PosCartLine {
  const PosCartLine({
    required this.productId,
    required this.quantity,
    this.modifierOptionIds = const [],
  });

  final String productId;
  final int quantity;
  final List<String> modifierOptionIds;

  @override
  int get hashCode =>
      Object.hash(productId, quantity, Object.hashAll(modifierOptionIds));

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PosCartLine &&
          runtimeType == other.runtimeType &&
          productId == other.productId &&
          quantity == other.quantity &&
          _sameStringList(modifierOptionIds, other.modifierOptionIds);
}

bool _sameStringList(List<String> left, List<String> right) =>
    left.length == right.length &&
    left.indexed.every((entry) => entry.$2 == right[entry.$1]);

enum _DiscountMode { fixed, percentage }

@immutable
class _OrderDiscountChoice {
  const _OrderDiscountChoice({
    this.fixedMinor,
    this.percentageBasisPoints,
    this.reason,
  });

  final int? fixedMinor;
  final int? percentageBasisPoints;
  final String? reason;
}

@immutable
class PosCheckoutRequest {
  const PosCheckoutRequest({
    required this.lines,
    required this.fulfillment,
    required this.paymentMethod,
    this.paymentAllocations,
    this.customerId,
    this.draftOrderId,
    this.expectedDraftRevision,
    this.discountFixedMinor,
    this.discountPercentageBasisPoints,
    this.discountPercentageCapMinor,
    this.discountReason,
  });

  final List<PosCartLine> lines;
  final String fulfillment;
  final String paymentMethod;
  final List<CommunityPaymentAllocation>? paymentAllocations;
  final String? customerId;
  final String? draftOrderId;
  final int? expectedDraftRevision;
  final int? discountFixedMinor;
  final int? discountPercentageBasisPoints;
  final int? discountPercentageCapMinor;
  final String? discountReason;
}

@immutable
class PosPricingPreview {
  const PosPricingPreview({
    required this.available,
    required this.status,
    required this.subtotalMinor,
    required this.discountMinor,
    required this.taxMinor,
    required this.payableMinor,
    this.currencyCode,
  });

  final bool available;
  final String status;
  final int subtotalMinor;
  final int discountMinor;
  final int taxMinor;
  final int payableMinor;
  final String? currencyCode;
}

typedef PreviewPosPricing =
    Future<PosPricingPreview> Function({
      required List<PosCartLine> lines,
      int? discountFixedMinor,
      int? discountPercentageBasisPoints,
      int? discountPercentageCapMinor,
      String? discountReason,
    });

/// The UI accepts a deliberately small, safe checkout result. Raw storage,
/// key-management, and database errors must never be shown at the counter.
@immutable
class PosCheckoutResult {
  const PosCheckoutResult({
    required this.recorded,
    required this.status,
    this.invoiceNumber,
    this.totalMinor,
    this.currencyCode,
    this.paymentMethod,
  });

  final bool recorded;
  final String status;
  final String? invoiceNumber;
  final int? totalMinor;
  final String? currencyCode;
  final String? paymentMethod;
}

typedef PersistPosCheckout =
    Future<PosCheckoutResult> Function(PosCheckoutRequest request);

typedef CreatePosCustomer =
    Future<void> Function({
      required String displayName,
      String? phoneNumber,
      String? emailAddress,
      required bool marketingConsent,
    });

@immutable
class PosDraftRequest {
  const PosDraftRequest({
    this.draftOrderId,
    this.expectedRevision,
    required this.lines,
    required this.fulfillment,
    this.tableName,
    this.kitchenNote,
  });

  final String? draftOrderId;
  final int? expectedRevision;
  final List<PosCartLine> lines;
  final String fulfillment;
  final String? tableName;
  final String? kitchenNote;
}

@immutable
class PosDraftResult {
  const PosDraftResult({
    required this.saved,
    required this.status,
    this.draftOrderId,
    this.revision,
  });

  final bool saved;
  final String status;
  final String? draftOrderId;
  final int? revision;
}

typedef PersistPosDraft =
    Future<PosDraftResult> Function(PosDraftRequest request);

@immutable
class PosDraftActionResult {
  const PosDraftActionResult({required this.succeeded, required this.status});

  final bool succeeded;
  final String status;
}

typedef SendPosDraftToKitchen =
    Future<PosDraftActionResult> Function(String draftOrderId, int revision);

/// Management actions against an order that has already reached the kitchen.
///
/// The storage boundary creates a retained cancellation notice and, for a
/// reopen, a new immutable revision. The UI intentionally receives only a
/// safe result rather than any mutable kitchen payload.
typedef ManageSentPosDraft =
    Future<PosDraftActionResult> Function(
      String draftOrderId,
      int revision,
      String reason,
    );

/// Community Edition's first counter workflow.
///
/// This is intentionally an immediate local sale, rather than a persistent
/// open-table or kitchen workflow. The cart exists only in memory until the
/// checkout callback atomically records its immutable order, invoice, payment,
/// audit events, and sync-outbox events in Rust.
class PosWorkspace extends StatefulWidget {
  const PosWorkspace({
    required this.workspace,
    this.canOperateCounter = true,
    this.canManageOrders = false,
    required this.isSaving,
    required this.onCheckout,
    required this.onPreviewPricing,
    required this.onSaveDraft,
    required this.onSendToKitchen,
    this.onCancelDraft,
    this.onCancelSentDraft,
    this.onReopenSentDraft,
    required this.onOpenMenu,
    this.onCreateCustomer,
    super.key,
  });

  final CommunityWorkspace workspace;
  final bool canOperateCounter;
  final bool canManageOrders;
  final bool isSaving;
  final PersistPosCheckout onCheckout;
  final PreviewPosPricing onPreviewPricing;
  final PersistPosDraft onSaveDraft;
  final SendPosDraftToKitchen onSendToKitchen;
  final Future<void> Function(String draftOrderId, int revision, String reason)?
  onCancelDraft;
  final ManageSentPosDraft? onCancelSentDraft;
  final ManageSentPosDraft? onReopenSentDraft;
  final VoidCallback onOpenMenu;
  final CreatePosCustomer? onCreateCustomer;

  @override
  State<PosWorkspace> createState() => _PosWorkspaceState();
}

class _PosWorkspaceState extends State<PosWorkspace> {
  final Map<_CartLineKey, int> _quantities = <_CartLineKey, int>{};
  final _searchController = TextEditingController();
  final _searchFocusNode = FocusNode(debugLabel: 'POS menu search');
  String _searchQuery = '';
  String? _selectedCategoryId;
  String _fulfillment = 'takeaway';
  String _paymentMethod = 'cash';
  String? _selectedCustomerId;
  String? _checkoutStatus;
  PosCheckoutResult? _lastReceipt;
  var _isSubmitting = false;
  var _showCompactCart = false;
  String? _draftOrderId;
  int? _draftRevision;
  String? _draftState;
  String? _tableName;
  String? _kitchenNote;
  var _draftNeedsSaving = false;

  bool get _isKitchenSent => _draftState == 'sent_to_kitchen';

  @override
  void dispose() {
    _searchController.dispose();
    _searchFocusNode.dispose();
    super.dispose();
  }

  @override
  void didUpdateWidget(covariant PosWorkspace oldWidget) {
    super.didUpdateWidget(oldWidget);

    final activeProductIds = widget.workspace.products
        .map((product) => product.productId)
        .toSet();
    _quantities.removeWhere(
      (key, quantity) =>
          !activeProductIds.contains(key.productId) || quantity <= 0,
    );

    final activeCategoryIds = widget.workspace.categories
        .map((category) => category.categoryId)
        .toSet();
    if (_selectedCategoryId != null &&
        !activeCategoryIds.contains(_selectedCategoryId)) {
      _selectedCategoryId = null;
    }
    final activeCustomerIds = widget.workspace.customers
        .map((customer) => customer.customerId)
        .toSet();
    if (_selectedCustomerId != null &&
        !activeCustomerIds.contains(_selectedCustomerId)) {
      _selectedCustomerId = null;
    }
  }

  bool get _storageNeedsAttention => widget.workspace.storageStatus.startsWith(
    'Local storage needs attention',
  );

  bool get _canSell =>
      !_storageNeedsAttention &&
      !widget.workspace.setupRequired &&
      widget.workspace.products.isNotEmpty;

  bool get _isBusy => widget.isSaving || _isSubmitting;

  List<_CartEntry> get _cartEntries {
    final productsById = <String, CommunityProductView>{
      for (final product in widget.workspace.products)
        product.productId: product,
    };
    final entries = <_CartEntry>[];
    for (final entry in _quantities.entries) {
      final product = productsById[entry.key.productId];
      if (product != null && entry.value > 0) {
        final optionsById = <String, CommunityModifierOptionView>{
          for (final option in product.modifierOptions)
            option.modifierOptionId: option,
        };
        entries.add(
          _CartEntry(
            key: entry.key,
            product: product,
            quantity: entry.value,
            modifierOptions: entry.key.modifierOptionIds
                .map((modifierOptionId) => optionsById[modifierOptionId])
                .whereType<CommunityModifierOptionView>()
                .toList(growable: false),
          ),
        );
      }
    }
    entries.sort(
      (left, right) =>
          left.product.displayName.compareTo(right.product.displayName),
    );
    return entries;
  }

  int get _cartItemCount =>
      _quantities.values.fold(0, (total, quantity) => total + quantity);

  int get _subtotalMinor =>
      _cartEntries.fold(0, (total, entry) => total + entry.lineTotalMinor);

  String get _currencyCode => widget.workspace.products.isEmpty
      ? 'INR'
      : widget.workspace.products.first.currencyCode;

  List<CommunityProductView> get _visibleProducts {
    final normalizedQuery = _searchQuery.trim().toLowerCase();
    return widget.workspace.products.where((product) {
      if (!product.isAvailable) {
        return false;
      }
      final belongsToSelectedCategory =
          _selectedCategoryId == null ||
          product.categoryId == _selectedCategoryId;
      final matchesQuery =
          normalizedQuery.isEmpty ||
          product.displayName.toLowerCase().contains(normalizedQuery);
      return belongsToSelectedCategory && matchesQuery;
    }).toList();
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.canOperateCounter) {
      return _PosUnavailable(
        icon: Icons.lock_outline,
        title: 'Counter is restricted',
        detail:
            'Unlock as cashier, a manager, or the owner to create and settle orders.',
      );
    }
    if (_storageNeedsAttention) {
      return _PosUnavailable(
        icon: Icons.shield_outlined,
        title: 'Counter is locked until local storage is ready',
        detail: widget.workspace.storageStatus,
        actionLabel: 'Open menu',
        onAction: widget.onOpenMenu,
      );
    }

    if (widget.workspace.setupRequired) {
      return _PosUnavailable(
        icon: Icons.storefront_outlined,
        title: 'Set up your restaurant before billing',
        detail:
            'Create your local restaurant workspace, then add a sellable menu item before starting the counter.',
        actionLabel: 'Set up restaurant',
        onAction: widget.onOpenMenu,
      );
    }

    if (widget.workspace.products.isEmpty) {
      return _PosUnavailable(
        icon: Icons.restaurant_menu_outlined,
        title: 'Add a menu item to begin',
        detail:
            'The counter only sells active menu items saved in your encrypted local workspace.',
        actionLabel: 'Open menu',
        onAction: widget.onOpenMenu,
      );
    }

    return CallbackShortcuts(
      bindings: <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.keyF, control: true):
            _focusMenuSearch,
      },
      child: Focus(
        autofocus: true,
        child: LayoutBuilder(
          builder: (context, constraints) {
            // The restaurant shell consumes 248 px with its desktop navigation.
            // Choose the POS presentation from the remaining content width so a
            // 960 px app window does not accidentally force a cramped desktop
            // counter into a roughly 700 px content pane.
            final isWide = constraints.maxWidth >= 880;
            if (isWide) {
              return Stack(
                children: [
                  _DesktopCounter(
                    workspace: widget.workspace,
                    visibleProducts: _visibleProducts,
                    selectedCategoryId: _selectedCategoryId,
                    searchQuery: _searchQuery,
                    searchController: _searchController,
                    searchFocusNode: _searchFocusNode,
                    canAddProducts: !_isKitchenSent,
                    cartEntries: _cartEntries,
                    cartItemCount: _cartItemCount,
                    subtotalMinor: _subtotalMinor,
                    currencyCode: _currencyCode,
                    fulfillment: _fulfillment,
                    paymentMethod: _paymentMethod,
                    kitchenNote: _kitchenNote,
                    customers: widget.workspace.customers,
                    selectedCustomerId: _selectedCustomerId,
                    isBusy: _isBusy,
                    checkoutStatus: _checkoutStatus,
                    receipt: _lastReceipt,
                    draftNeedsSaving: _draftNeedsSaving,
                    onSearchChanged: _setSearchQuery,
                    onCategoryChanged: _setCategory,
                    onAdd: _addProduct,
                    onQuantityChanged: _changeQuantity,
                    onFulfillmentChanged: _setFulfillment,
                    onPaymentMethodChanged: _setPaymentMethod,
                    onCustomerChanged: _setCustomer,
                    onAddCustomer: widget.onCreateCustomer == null
                        ? null
                        : _showCreateCustomerDialog,
                    onCheckout: _checkout,
                    onSaveDraft: _saveDraft,
                    onEditKitchenNote: _isKitchenSent ? null : _editKitchenNote,
                    isKitchenSent: _isKitchenSent,
                    canManageOrders: widget.canManageOrders,
                    onSendToKitchen:
                        _draftOrderId != null &&
                            _draftRevision != null &&
                            !_isKitchenSent
                        ? _sendToKitchen
                        : null,
                    onCancelSentDraft: widget.canManageOrders && _isKitchenSent
                        ? _cancelSentDraft
                        : null,
                    onReopenSentDraft: widget.canManageOrders && _isKitchenSent
                        ? _reopenSentDraft
                        : null,
                  ),
                  if (widget.workspace.openDrafts.isNotEmpty)
                    Positioned(
                      top: 16,
                      right: 28,
                      child: FilledButton.tonalIcon(
                        onPressed: _showOpenDrafts,
                        icon: const Icon(Icons.pending_actions_outlined),
                        label: Text(
                          'Open & sent orders (${widget.workspace.openDrafts.length})',
                        ),
                      ),
                    ),
                ],
              );
            }

            if (_showCompactCart) {
              return _CompactOrderScreen(
                cartEntries: _cartEntries,
                cartItemCount: _cartItemCount,
                subtotalMinor: _subtotalMinor,
                currencyCode: _currencyCode,
                fulfillment: _fulfillment,
                paymentMethod: _paymentMethod,
                kitchenNote: _kitchenNote,
                customers: widget.workspace.customers,
                selectedCustomerId: _selectedCustomerId,
                isBusy: _isBusy,
                checkoutStatus: _checkoutStatus,
                receipt: _lastReceipt,
                draftNeedsSaving: _draftNeedsSaving,
                onBack: () => setState(() => _showCompactCart = false),
                onQuantityChanged: _changeQuantity,
                onFulfillmentChanged: _setFulfillment,
                onPaymentMethodChanged: _setPaymentMethod,
                onCustomerChanged: _setCustomer,
                onAddCustomer: widget.onCreateCustomer == null
                    ? null
                    : _showCreateCustomerDialog,
                onCheckout: _checkout,
                onSaveDraft: _saveDraft,
                onEditKitchenNote: _isKitchenSent ? null : _editKitchenNote,
                isKitchenSent: _isKitchenSent,
                canManageOrders: widget.canManageOrders,
                onSendToKitchen:
                    _draftOrderId != null &&
                        _draftRevision != null &&
                        !_isKitchenSent
                    ? _sendToKitchen
                    : null,
                onCancelSentDraft: widget.canManageOrders && _isKitchenSent
                    ? _cancelSentDraft
                    : null,
                onReopenSentDraft: widget.canManageOrders && _isKitchenSent
                    ? _reopenSentDraft
                    : null,
              );
            }

            return _CompactCounter(
              workspace: widget.workspace,
              visibleProducts: _visibleProducts,
              selectedCategoryId: _selectedCategoryId,
              searchQuery: _searchQuery,
              searchController: _searchController,
              searchFocusNode: _searchFocusNode,
              cartItemCount: _cartItemCount,
              subtotalMinor: _subtotalMinor,
              currencyCode: _currencyCode,
              canAddProducts: !_isKitchenSent,
              onSearchChanged: _setSearchQuery,
              onCategoryChanged: _setCategory,
              onAdd: _addProduct,
              openDraftCount: widget.workspace.openDrafts.length,
              onOpenDrafts: widget.workspace.openDrafts.isEmpty
                  ? null
                  : _showOpenDrafts,
              onViewCart: () => setState(() => _showCompactCart = true),
            );
          },
        ),
      ),
    );
  }

  void _focusMenuSearch() {
    void requestFocus() {
      if (!mounted) return;
      _searchFocusNode.requestFocus();
      _searchController.selection = TextSelection(
        baseOffset: 0,
        extentOffset: _searchController.text.length,
      );
    }

    if (_showCompactCart) {
      setState(() => _showCompactCart = false);
      WidgetsBinding.instance.addPostFrameCallback((_) => requestFocus());
    } else {
      requestFocus();
    }
  }

  Future<void> _showOpenDrafts() async {
    final draft = await showModalBottomSheet<CommunityDraftOrderView>(
      context: context,
      showDragHandle: true,
      builder: (context) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
          children: [
            Text(
              'Open & sent orders',
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 12),
            for (final item in widget.workspace.openDrafts)
              Builder(
                builder: (context) {
                  final isKitchenSent = item.draftState == 'sent_to_kitchen';
                  return ListTile(
                    leading: Icon(
                      isKitchenSent
                          ? Icons.soup_kitchen_outlined
                          : Icons.table_restaurant_outlined,
                    ),
                    title: Text(item.tableName ?? 'Takeaway order'),
                    subtitle: Text(
                      '${isKitchenSent ? 'Sent to kitchen • ' : ''}${item.lineCount} item${item.lineCount == 1 ? '' : 's'} • ${formatMinorPrice(item.subtotalMinor, item.currencyCode)}',
                    ),
                    trailing: isKitchenSent
                        ? const Tooltip(
                            message: 'Items are locked after kitchen send',
                            child: Icon(Icons.lock_outline),
                          )
                        : widget.onCancelDraft == null
                        ? const Icon(Icons.arrow_forward)
                        : IconButton(
                            tooltip: 'Cancel open order',
                            icon: const Icon(Icons.cancel_outlined),
                            onPressed: () async {
                              final reason = await _requestCancellationReason();
                              if (!context.mounted || reason == null) return;
                              Navigator.of(context).pop();
                              await widget.onCancelDraft!(
                                item.draftOrderId,
                                item.revision,
                                reason,
                              );
                            },
                          ),
                    onTap: () => Navigator.of(context).pop(item),
                  );
                },
              ),
          ],
        ),
      ),
    );
    if (!mounted || draft == null) return;
    setState(() {
      _quantities
        ..clear()
        ..addEntries(
          draft.lines.map(
            (line) => MapEntry(
              _CartLineKey(line.productId, line.modifierOptionIds),
              line.quantity,
            ),
          ),
        );
      _draftOrderId = draft.draftOrderId;
      _draftRevision = draft.revision;
      _draftState = draft.draftState;
      _fulfillment = draft.fulfillment;
      _tableName = draft.tableName;
      _kitchenNote = draft.kitchenNote;
      _draftNeedsSaving = false;
      _checkoutStatus = _isKitchenSent
          ? 'Kitchen-sent order restored. Its items are locked; record payment when ready.'
          : 'Open order restored locally.';
      _lastReceipt = null;
      _showCompactCart = true;
    });
  }

  Future<String?> _requestCancellationReason() async {
    final controller = TextEditingController();
    return showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Cancel open order?'),
        content: TextField(
          controller: controller,
          autofocus: true,
          maxLength: 500,
          decoration: const InputDecoration(
            labelText: 'Reason',
            helperText: 'This keeps the order history for reconciliation.',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Keep order'),
          ),
          FilledButton.tonal(
            onPressed: () {
              final reason = controller.text.trim();
              if (reason.length >= 3) Navigator.of(dialogContext).pop(reason);
            },
            child: const Text('Cancel order'),
          ),
        ],
      ),
    );
  }

  Future<void> _sendToKitchen() async {
    final draftOrderId = _draftOrderId;
    final revision = _draftRevision;
    if (_isBusy || _isKitchenSent || draftOrderId == null || revision == null) {
      return;
    }
    setState(() => _isSubmitting = true);
    try {
      PosDraftActionResult result;
      try {
        result = await widget.onSendToKitchen(draftOrderId, revision);
      } catch (_) {
        result = const PosDraftActionResult(
          succeeded: false,
          status:
              'Kitchen needs attention • the order was not sent. It is still held locally.',
        );
      }
      if (mounted) {
        setState(() {
          _checkoutStatus = result.status;
          if (result.succeeded) {
            _draftState = 'sent_to_kitchen';
          }
        });
      }
    } finally {
      if (mounted) {
        setState(() => _isSubmitting = false);
      }
    }
  }

  Future<void> _cancelSentDraft() => _manageSentDraft(
    title: 'Cancel kitchen-sent order?',
    helperText:
        'The kitchen will receive a cancellation notice. The order remains in audit history.',
    confirmationLabel: 'Request cancellation',
    successDetail:
        'The cancellation was recorded. The kitchen must acknowledge it before the ticket is cleared.',
    action: widget.onCancelSentDraft,
  );

  Future<void> _reopenSentDraft() => _manageSentDraft(
    title: 'Reopen as a new revision?',
    helperText:
        'The kitchen will receive a cancellation notice. A new order revision can then be edited and sent again.',
    confirmationLabel: 'Reopen order',
    successDetail:
        'A new revision was opened. Select it from Open & sent orders before editing.',
    action: widget.onReopenSentDraft,
  );

  Future<void> _manageSentDraft({
    required String title,
    required String helperText,
    required String confirmationLabel,
    required String successDetail,
    required ManageSentPosDraft? action,
  }) async {
    final draftOrderId = _draftOrderId;
    final revision = _draftRevision;
    if (_isBusy ||
        !_isKitchenSent ||
        !widget.canManageOrders ||
        action == null ||
        draftOrderId == null ||
        revision == null) {
      return;
    }

    final reason = await _requestSentOrderManagementReason(
      title: title,
      helperText: helperText,
      confirmationLabel: confirmationLabel,
    );
    if (!mounted || reason == null) return;

    setState(() => _isSubmitting = true);
    PosDraftActionResult result;
    try {
      result = await action(draftOrderId, revision, reason);
    } catch (_) {
      result = const PosDraftActionResult(
        succeeded: false,
        status:
            'The order needs attention. Its kitchen state could not be changed locally.',
      );
    }

    if (!mounted) return;
    setState(() {
      _isSubmitting = false;
      _checkoutStatus = result.succeeded
          ? '${result.status} $successDetail'
          : result.status;
      if (result.succeeded) {
        // Do not guess the newly created revision. The refreshed workspace is
        // the source of truth; the manager explicitly restores it from the
        // retained order list before making any change.
        _quantities.clear();
        _draftOrderId = null;
        _draftRevision = null;
        _draftState = null;
        _tableName = null;
        _kitchenNote = null;
        _draftNeedsSaving = false;
      }
    });
  }

  Future<String?> _requestSentOrderManagementReason({
    required String title,
    required String helperText,
    required String confirmationLabel,
  }) async {
    final controller = TextEditingController();
    try {
      return await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: Text(title),
          content: TextField(
            controller: controller,
            autofocus: true,
            maxLength: 500,
            textCapitalization: TextCapitalization.sentences,
            decoration: InputDecoration(
              labelText: 'Manager reason',
              helperText: helperText,
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Keep order'),
            ),
            FilledButton.tonal(
              onPressed: () {
                final reason = controller.text.trim();
                if (reason.length >= 3) {
                  Navigator.of(dialogContext).pop(reason);
                }
              },
              child: Text(confirmationLabel),
            ),
          ],
        ),
      );
    } finally {
      controller.dispose();
    }
  }

  void _setSearchQuery(String value) {
    setState(() {
      _searchQuery = value;
    });
  }

  void _setCategory(String? categoryId) {
    setState(() {
      _selectedCategoryId = categoryId;
    });
  }

  void _addProduct(CommunityProductView product) {
    if (!_canSell || _isBusy) {
      return;
    }
    if (_isKitchenSent) {
      _showKitchenSentLockMessage();
      return;
    }
    final selectableModifiers = product.modifierOptions
        .where((option) => !option.archived)
        .toList(growable: false);
    if (selectableModifiers.isEmpty) {
      _addProductWithModifiers(product, const []);
      return;
    }
    _showModifierSelector(product, selectableModifiers);
  }

  Future<void> _showModifierSelector(
    CommunityProductView product,
    List<CommunityModifierOptionView> selectableModifiers,
  ) async {
    final selectedModifierIds = await showDialog<List<String>>(
      context: context,
      builder: (dialogContext) {
        final selected = <String>{};
        return StatefulBuilder(
          builder: (dialogContext, setDialogState) => AlertDialog(
            title: Text('Customise ${product.displayName}'),
            content: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420, maxHeight: 420),
              child: SingleChildScrollView(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      'Choose up to $_maximumModifierOptionsPerSaleLine optional additions. Prices are verified again by the local Rust checkout.',
                      style: Theme.of(dialogContext).textTheme.bodySmall,
                    ),
                    const SizedBox(height: 8),
                    for (final option in selectableModifiers)
                      CheckboxListTile(
                        contentPadding: EdgeInsets.zero,
                        value: selected.contains(option.modifierOptionId),
                        onChanged:
                            !selected.contains(option.modifierOptionId) &&
                                selected.length >=
                                    _maximumModifierOptionsPerSaleLine
                            ? null
                            : (selectedValue) => setDialogState(() {
                                if (selectedValue ?? false) {
                                  selected.add(option.modifierOptionId);
                                } else {
                                  selected.remove(option.modifierOptionId);
                                }
                              }),
                        title: Text(option.displayName),
                        subtitle: Text(
                          option.priceDeltaMinor == 0
                              ? 'Included'
                              : '+${formatMinorPrice(option.priceDeltaMinor, option.currencyCode)}',
                        ),
                      ),
                  ],
                ),
              ),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(dialogContext).pop(),
                child: const Text('Cancel'),
              ),
              FilledButton(
                onPressed: () => Navigator.of(
                  dialogContext,
                ).pop(selected.toList(growable: false)),
                child: const Text('Add to order'),
              ),
            ],
          ),
        );
      },
    );
    if (!mounted || selectedModifierIds == null) return;
    _addProductWithModifiers(product, selectedModifierIds);
  }

  void _addProductWithModifiers(
    CommunityProductView product,
    List<String> modifierOptionIds,
  ) {
    if (!_canSell || _isBusy || _isKitchenSent) return;
    final key = _CartLineKey(product.productId, modifierOptionIds);
    setState(() {
      _quantities.update(key, (quantity) => quantity + 1, ifAbsent: () => 1);
      _checkoutStatus = null;
      _lastReceipt = null;
      _draftOrderId = null;
      _draftRevision = null;
      _draftState = null;
      _draftNeedsSaving = false;
    });
  }

  void _changeQuantity(_CartEntry entry, int delta) {
    if (_isBusy) {
      return;
    }
    if (_isKitchenSent) {
      _showKitchenSentLockMessage();
      return;
    }
    setState(() {
      final nextQuantity = (_quantities[entry.key] ?? 0) + delta;
      if (nextQuantity <= 0) {
        _quantities.remove(entry.key);
      } else {
        _quantities[entry.key] = nextQuantity;
      }
      _checkoutStatus = null;
      _lastReceipt = null;
      _draftOrderId = null;
      _draftRevision = null;
      _draftState = null;
      _draftNeedsSaving = false;
    });
  }

  void _setFulfillment(String value) {
    if (_isKitchenSent) {
      _showKitchenSentLockMessage();
      return;
    }
    if (_fulfillment == value) return;
    setState(() {
      _fulfillment = value;
      if (_draftOrderId != null) {
        _draftNeedsSaving = true;
      }
    });
  }

  void _setPaymentMethod(String value) {
    setState(() {
      _paymentMethod = value;
    });
  }

  void _setCustomer(String? customerId) {
    setState(() {
      _selectedCustomerId = customerId;
    });
  }

  Future<void> _showCreateCustomerDialog() async {
    final submit = widget.onCreateCustomer;
    if (submit == null) return;
    final nameController = TextEditingController();
    final phoneController = TextEditingController();
    final emailController = TextEditingController();
    var marketingConsent = false;
    try {
      await showDialog<void>(
        context: context,
        builder: (dialogContext) => StatefulBuilder(
          builder: (dialogContext, setDialogState) => AlertDialog(
            title: const Text('Add customer'),
            content: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  TextField(
                    controller: nameController,
                    autofocus: true,
                    textCapitalization: TextCapitalization.words,
                    decoration: const InputDecoration(labelText: 'Name *'),
                  ),
                  TextField(
                    controller: phoneController,
                    keyboardType: TextInputType.phone,
                    decoration: const InputDecoration(
                      labelText: 'Phone (optional)',
                    ),
                  ),
                  TextField(
                    controller: emailController,
                    keyboardType: TextInputType.emailAddress,
                    decoration: const InputDecoration(
                      labelText: 'Email (optional)',
                    ),
                  ),
                  CheckboxListTile(
                    contentPadding: EdgeInsets.zero,
                    value: marketingConsent,
                    onChanged: (value) =>
                        setDialogState(() => marketingConsent = value ?? false),
                    title: const Text('Marketing consent'),
                    subtitle: const Text(
                      'Only select with the customer\'s permission.',
                    ),
                  ),
                ],
              ),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(dialogContext).pop(),
                child: const Text('Cancel'),
              ),
              FilledButton(
                onPressed: () async {
                  final name = nameController.text.trim();
                  if (name.isEmpty) return;
                  await submit(
                    displayName: name,
                    phoneNumber: _optionalText(phoneController.text),
                    emailAddress: _optionalText(emailController.text),
                    marketingConsent: marketingConsent,
                  );
                  if (dialogContext.mounted) Navigator.of(dialogContext).pop();
                },
                child: const Text('Save customer'),
              ),
            ],
          ),
        ),
      );
    } finally {
      nameController.dispose();
      phoneController.dispose();
      emailController.dispose();
    }
  }

  String? _optionalText(String value) {
    final trimmed = value.trim();
    return trimmed.isEmpty ? null : trimmed;
  }

  Future<void> _checkout() async {
    if (_isBusy || _cartEntries.isEmpty) {
      return;
    }

    var discountFixedMinor = null as int?;
    var discountPercentageBasisPoints = null as int?;
    var discountReason = null as String?;
    if (widget.canManageOrders) {
      final discountChoice = await _requestOptionalOrderDiscount();
      if (!mounted || discountChoice == null) {
        return;
      }
      discountFixedMinor = discountChoice.fixedMinor;
      discountPercentageBasisPoints = discountChoice.percentageBasisPoints;
      discountReason = discountChoice.reason;
    }

    final cartLines = _cartEntries
        .map(
          (entry) => PosCartLine(
            productId: entry.product.productId,
            quantity: entry.quantity,
            modifierOptionIds: entry.key.modifierOptionIds,
          ),
        )
        .toList(growable: false);

    final preview = await widget.onPreviewPricing(
      lines: cartLines,
      discountFixedMinor: discountFixedMinor,
      discountPercentageBasisPoints: discountPercentageBasisPoints,
      discountReason: discountReason,
    );
    if (!mounted) {
      return;
    }
    if (!preview.available || preview.payableMinor <= 0) {
      setState(() => _checkoutStatus = preview.status);
      return;
    }

    final paymentAllocations = _paymentMethod == 'split'
        ? await _requestSplitPaymentAllocations(preview.payableMinor)
        : null;
    if (!mounted || (_paymentMethod == 'split' && paymentAllocations == null)) {
      return;
    }

    final request = PosCheckoutRequest(
      lines: cartLines,
      fulfillment: _fulfillment,
      paymentMethod: paymentAllocations?.first.paymentMethod ?? _paymentMethod,
      paymentAllocations: paymentAllocations,
      customerId: _selectedCustomerId,
      draftOrderId: _draftOrderId,
      expectedDraftRevision: _draftRevision,
      discountFixedMinor: discountFixedMinor,
      discountPercentageBasisPoints: discountPercentageBasisPoints,
      discountReason: discountReason,
    );

    setState(() {
      _isSubmitting = true;
      _checkoutStatus = null;
    });

    PosCheckoutResult result;
    try {
      result = await widget.onCheckout(request);
    } catch (_) {
      result = const PosCheckoutResult(
        recorded: false,
        status:
            'Sale needs attention • the local sale could not be recorded. Your cart is still here.',
      );
    }

    if (!mounted) {
      return;
    }

    setState(() {
      _isSubmitting = false;
      _checkoutStatus = result.status;
      if (result.recorded) {
        _quantities.clear();
        _selectedCustomerId = null;
        _draftOrderId = null;
        _draftRevision = null;
        _draftState = null;
        _kitchenNote = null;
        _draftNeedsSaving = false;
        _lastReceipt = result;
        _showCompactCart = false;
      }
    });

    if (result.recorded) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            result.invoiceNumber == null
                ? 'Sale saved locally.'
                : 'Invoice ${result.invoiceNumber} saved locally.',
          ),
        ),
      );
    }
  }

  Future<List<CommunityPaymentAllocation>?> _requestSplitPaymentAllocations(
    int payableMinor,
  ) async {
    return showDialog<List<CommunityPaymentAllocation>>(
      context: context,
      barrierDismissible: false,
      builder: (_) => _SplitPaymentDialog(
        totalMinor: payableMinor,
        currencyCode: _currencyCode,
      ),
    );
  }

  /// Null aborts checkout. A choice with neither fixed nor percentage amount
  /// continues without a discount.
  Future<_OrderDiscountChoice?> _requestOptionalOrderDiscount() async {
    final amountController = TextEditingController();
    final percentController = TextEditingController();
    final reasonController = TextEditingController();
    var mode = _DiscountMode.fixed;
    try {
      return await showDialog<_OrderDiscountChoice>(
        context: context,
        barrierDismissible: false,
        builder: (dialogContext) => StatefulBuilder(
          builder: (dialogContext, setDialogState) => AlertDialog(
            title: const Text('Order discount'),
            content: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Text(
                  'Optional owner/manager discount. Leave blank to continue without one. Rust revalidates authority and recalculates tax before recording.',
                ),
                const SizedBox(height: 12),
                SegmentedButton<_DiscountMode>(
                  segments: const [
                    ButtonSegment(
                      value: _DiscountMode.fixed,
                      label: Text('Fixed'),
                    ),
                    ButtonSegment(
                      value: _DiscountMode.percentage,
                      label: Text('Percent'),
                    ),
                  ],
                  selected: {mode},
                  onSelectionChanged: (selected) {
                    setDialogState(() => mode = selected.first);
                  },
                ),
                const SizedBox(height: 12),
                if (mode == _DiscountMode.fixed)
                  TextField(
                    controller: amountController,
                    keyboardType: const TextInputType.numberWithOptions(
                      decimal: true,
                    ),
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(r'[0-9.,]')),
                    ],
                    decoration: InputDecoration(
                      labelText: 'Discount amount ($_currencyCode)',
                    ),
                  )
                else
                  TextField(
                    controller: percentController,
                    keyboardType: const TextInputType.numberWithOptions(
                      decimal: true,
                    ),
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(r'[0-9.]')),
                    ],
                    decoration: const InputDecoration(
                      labelText: 'Discount percent',
                      helperText: 'Example: 10 for 10%',
                    ),
                  ),
                const SizedBox(height: 8),
                TextField(
                  controller: reasonController,
                  maxLength: 500,
                  textCapitalization: TextCapitalization.sentences,
                  decoration: const InputDecoration(labelText: 'Reason'),
                ),
              ],
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(dialogContext).pop(),
                child: const Text('Cancel'),
              ),
              TextButton(
                onPressed: () => Navigator.of(
                  dialogContext,
                ).pop(const _OrderDiscountChoice()),
                child: const Text('No discount'),
              ),
              FilledButton(
                onPressed: () {
                  final reason = reasonController.text.trim();
                  if (mode == _DiscountMode.fixed) {
                    final amountText = amountController.text.trim();
                    if (amountText.isEmpty) {
                      Navigator.of(
                        dialogContext,
                      ).pop(const _OrderDiscountChoice());
                      return;
                    }
                    final amountMinor = parseDecimalPriceToMinorUnits(
                      amountText,
                    );
                    if (amountMinor == null ||
                        amountMinor <= 0 ||
                        reason.length < 3) {
                      return;
                    }
                    Navigator.of(dialogContext).pop(
                      _OrderDiscountChoice(
                        fixedMinor: amountMinor,
                        reason: reason,
                      ),
                    );
                    return;
                  }
                  final percentText = percentController.text.trim();
                  if (percentText.isEmpty) {
                    Navigator.of(
                      dialogContext,
                    ).pop(const _OrderDiscountChoice());
                    return;
                  }
                  final percent = double.tryParse(percentText);
                  if (percent == null ||
                      percent <= 0 ||
                      percent > 100 ||
                      reason.length < 3) {
                    return;
                  }
                  final basisPoints = (percent * 100).round();
                  Navigator.of(dialogContext).pop(
                    _OrderDiscountChoice(
                      percentageBasisPoints: basisPoints,
                      reason: reason,
                    ),
                  );
                },
                child: const Text('Apply discount'),
              ),
            ],
          ),
        ),
      );
    } finally {
      amountController.dispose();
      percentController.dispose();
      reasonController.dispose();
    }
  }

  Future<void> _saveDraft() async {
    if (_isBusy || _cartEntries.isEmpty) return;
    if (_isKitchenSent) {
      _showKitchenSentLockMessage();
      return;
    }
    if (_fulfillment == 'dine_in' &&
        (_tableName == null || _tableName!.isEmpty)) {
      final tableName = await _requestTableName();
      if (!mounted || tableName == null) return;
      _tableName = tableName;
    }
    setState(() {
      _isSubmitting = true;
      _checkoutStatus = null;
    });
    PosDraftResult result;
    try {
      result = await widget.onSaveDraft(
        PosDraftRequest(
          draftOrderId: _draftOrderId,
          expectedRevision: _draftRevision,
          lines: _cartEntries
              .map(
                (entry) => PosCartLine(
                  productId: entry.product.productId,
                  quantity: entry.quantity,
                  modifierOptionIds: entry.key.modifierOptionIds,
                ),
              )
              .toList(growable: false),
          fulfillment: _fulfillment,
          tableName: _fulfillment == 'dine_in' ? _tableName : null,
          kitchenNote: _kitchenNote,
        ),
      );
    } catch (_) {
      result = const PosDraftResult(
        saved: false,
        status:
            'Open order needs attention • it could not be held locally. Your cart is still here.',
      );
    }
    if (!mounted) return;
    setState(() {
      _isSubmitting = false;
      _checkoutStatus = result.status;
      if (result.saved) {
        _draftOrderId = result.draftOrderId;
        _draftRevision = result.revision;
        _draftState = 'open';
        _draftNeedsSaving = false;
      }
    });
  }

  Future<void> _editKitchenNote() async {
    if (_isBusy || _isKitchenSent) return;
    final change = await showDialog<_KitchenNoteEdit>(
      context: context,
      builder: (_) => _KitchenNoteDialog(initialNote: _kitchenNote),
    );
    if (!mounted || change == null) return;
    setState(() {
      _kitchenNote = change.note;
      _checkoutStatus = null;
      if (_draftOrderId != null) {
        _draftNeedsSaving = true;
      }
    });
  }

  void _showKitchenSentLockMessage() {
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text(
          'This order has been sent to the kitchen. Reopen it through an authorized revision workflow before changing items.',
        ),
      ),
    );
  }

  Future<String?> _requestTableName() async {
    final controller = TextEditingController(text: _tableName ?? '');
    return showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Choose table'),
        content: TextField(
          controller: controller,
          autofocus: true,
          textCapitalization: TextCapitalization.words,
          decoration: const InputDecoration(
            labelText: 'Table name or number',
            hintText: 'For example, Table 7',
          ),
          onSubmitted: (value) {
            final table = value.trim();
            if (table.isNotEmpty) {
              Navigator.of(dialogContext).pop(table);
            }
          },
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () {
              final table = controller.text.trim();
              if (table.isNotEmpty) {
                Navigator.of(dialogContext).pop(table);
              }
            },
            child: const Text('Continue'),
          ),
        ],
      ),
    );
  }
}

@immutable
class _KitchenNoteEdit {
  const _KitchenNoteEdit({required this.note});

  final String? note;
}

class _KitchenNoteDialog extends StatefulWidget {
  const _KitchenNoteDialog({this.initialNote});

  final String? initialNote;

  @override
  State<_KitchenNoteDialog> createState() => _KitchenNoteDialogState();
}

class _KitchenNoteDialogState extends State<_KitchenNoteDialog> {
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: widget.initialNote ?? '');
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: const Text('Kitchen instruction'),
    content: TextField(
      key: const Key('pos-kitchen-note'),
      controller: _controller,
      autofocus: true,
      minLines: 3,
      maxLines: 5,
      maxLength: 500,
      textCapitalization: TextCapitalization.sentences,
      inputFormatters: [
        FilteringTextInputFormatter.deny(RegExp(r'[\x00-\x1F\x7F]')),
      ],
      decoration: const InputDecoration(
        labelText: 'Instruction for kitchen',
        helperText:
            'Saved with this order revision and shown only on Kitchen Display.',
        alignLabelWithHint: true,
      ),
    ),
    actions: [
      if (widget.initialNote != null)
        TextButton(
          key: const Key('pos-clear-kitchen-note'),
          onPressed: () =>
              Navigator.of(context).pop(const _KitchenNoteEdit(note: null)),
          child: const Text('Clear'),
        ),
      TextButton(
        onPressed: () => Navigator.of(context).pop(),
        child: const Text('Cancel'),
      ),
      FilledButton(
        key: const Key('pos-save-kitchen-note'),
        onPressed: () {
          final note = _controller.text.trim();
          Navigator.of(
            context,
          ).pop(_KitchenNoteEdit(note: note.isEmpty ? null : note));
        },
        child: const Text('Save instruction'),
      ),
    ],
  );
}

class _SplitPaymentDialog extends StatefulWidget {
  const _SplitPaymentDialog({
    required this.totalMinor,
    required this.currencyCode,
  });

  final int totalMinor;
  final String currencyCode;

  @override
  State<_SplitPaymentDialog> createState() => _SplitPaymentDialogState();
}

class _SplitPaymentDialogState extends State<_SplitPaymentDialog> {
  final _cashController = TextEditingController();
  final _cardController = TextEditingController();
  final _upiController = TextEditingController();
  String? _error;

  @override
  void dispose() {
    _cashController.dispose();
    _cardController.dispose();
    _upiController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: const Text('Split payment'),
    content: SingleChildScrollView(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Allocate exactly ${formatMinorPrice(widget.totalMinor, widget.currencyCode)}.',
          ),
          const SizedBox(height: 12),
          _amountField(_cashController, 'Cash', Icons.payments_outlined),
          _amountField(_cardController, 'Card', Icons.credit_card_outlined),
          _amountField(_upiController, 'UPI', Icons.qr_code_2_outlined),
          if (_error != null) ...[
            const SizedBox(height: 8),
            Text(
              _error!,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ],
        ],
      ),
    ),
    actions: [
      TextButton(
        onPressed: () => Navigator.of(context).pop(),
        child: const Text('Cancel'),
      ),
      FilledButton(onPressed: _submit, child: const Text('Use split payment')),
    ],
  );

  Widget _amountField(
    TextEditingController controller,
    String label,
    IconData icon,
  ) => TextField(
    controller: controller,
    keyboardType: const TextInputType.numberWithOptions(decimal: true),
    decoration: InputDecoration(labelText: label, prefixIcon: Icon(icon)),
  );

  void _submit() {
    final values = <(String, int)>[
      ('cash', parseDecimalPriceToMinorUnits(_cashController.text) ?? 0),
      ('card', parseDecimalPriceToMinorUnits(_cardController.text) ?? 0),
      ('upi', parseDecimalPriceToMinorUnits(_upiController.text) ?? 0),
    ].where((entry) => entry.$2 > 0).toList(growable: false);
    final total = values.fold<int>(0, (sum, entry) => sum + entry.$2);
    if (values.isEmpty || total != widget.totalMinor) {
      setState(
        () => _error =
            'Enter positive amounts that total exactly ${formatMinorPrice(widget.totalMinor, widget.currencyCode)}.',
      );
      return;
    }
    Navigator.of(context).pop(
      values
          .map(
            (entry) => CommunityPaymentAllocation(
              paymentMethod: entry.$1,
              amountMinor: entry.$2,
            ),
          )
          .toList(growable: false),
    );
  }
}

class _DesktopCounter extends StatelessWidget {
  const _DesktopCounter({
    required this.workspace,
    required this.visibleProducts,
    required this.selectedCategoryId,
    required this.searchQuery,
    required this.searchController,
    required this.searchFocusNode,
    required this.canAddProducts,
    required this.cartEntries,
    required this.cartItemCount,
    required this.subtotalMinor,
    required this.currencyCode,
    required this.fulfillment,
    required this.paymentMethod,
    this.kitchenNote,
    required this.customers,
    required this.selectedCustomerId,
    required this.isBusy,
    required this.checkoutStatus,
    required this.receipt,
    required this.draftNeedsSaving,
    required this.onSearchChanged,
    required this.onCategoryChanged,
    required this.onAdd,
    required this.onQuantityChanged,
    required this.onFulfillmentChanged,
    required this.onPaymentMethodChanged,
    required this.onCustomerChanged,
    this.onAddCustomer,
    required this.onCheckout,
    required this.onSaveDraft,
    this.onEditKitchenNote,
    required this.isKitchenSent,
    required this.canManageOrders,
    this.onSendToKitchen,
    this.onCancelSentDraft,
    this.onReopenSentDraft,
  });

  final CommunityWorkspace workspace;
  final List<CommunityProductView> visibleProducts;
  final String? selectedCategoryId;
  final String searchQuery;
  final TextEditingController searchController;
  final FocusNode searchFocusNode;
  final bool canAddProducts;
  final List<_CartEntry> cartEntries;
  final int cartItemCount;
  final int subtotalMinor;
  final String currencyCode;
  final String fulfillment;
  final String paymentMethod;
  final String? kitchenNote;
  final List<CommunityCustomerView> customers;
  final String? selectedCustomerId;
  final bool isBusy;
  final String? checkoutStatus;
  final PosCheckoutResult? receipt;
  final bool draftNeedsSaving;
  final ValueChanged<String> onSearchChanged;
  final ValueChanged<String?> onCategoryChanged;
  final ValueChanged<CommunityProductView> onAdd;
  final void Function(_CartEntry entry, int delta) onQuantityChanged;
  final ValueChanged<String> onFulfillmentChanged;
  final ValueChanged<String> onPaymentMethodChanged;
  final ValueChanged<String?> onCustomerChanged;
  final VoidCallback? onAddCustomer;
  final Future<void> Function() onCheckout;
  final Future<void> Function() onSaveDraft;
  final VoidCallback? onEditKitchenNote;
  final bool isKitchenSent;
  final bool canManageOrders;
  final Future<void> Function()? onSendToKitchen;
  final Future<void> Function()? onCancelSentDraft;
  final Future<void> Function()? onReopenSentDraft;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(36, 28, 28, 28),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: _CatalogPane(
              workspace: workspace,
              visibleProducts: visibleProducts,
              selectedCategoryId: selectedCategoryId,
              searchQuery: searchQuery,
              searchController: searchController,
              searchFocusNode: searchFocusNode,
              onSearchChanged: onSearchChanged,
              onCategoryChanged: onCategoryChanged,
              onAdd: onAdd,
              canAddProducts: canAddProducts,
            ),
          ),
          const SizedBox(width: 20),
          SizedBox(
            width: 390,
            child: _CartPanel(
              compact: false,
              cartEntries: cartEntries,
              cartItemCount: cartItemCount,
              subtotalMinor: subtotalMinor,
              currencyCode: currencyCode,
              fulfillment: fulfillment,
              paymentMethod: paymentMethod,
              kitchenNote: kitchenNote,
              customers: customers,
              selectedCustomerId: selectedCustomerId,
              isBusy: isBusy,
              checkoutStatus: checkoutStatus,
              receipt: receipt,
              draftNeedsSaving: draftNeedsSaving,
              onQuantityChanged: onQuantityChanged,
              onFulfillmentChanged: onFulfillmentChanged,
              onPaymentMethodChanged: onPaymentMethodChanged,
              onCustomerChanged: onCustomerChanged,
              onAddCustomer: onAddCustomer,
              onCheckout: onCheckout,
              onSaveDraft: onSaveDraft,
              onEditKitchenNote: onEditKitchenNote,
              isKitchenSent: isKitchenSent,
              canManageOrders: canManageOrders,
              onSendToKitchen: onSendToKitchen,
              onCancelSentDraft: onCancelSentDraft,
              onReopenSentDraft: onReopenSentDraft,
            ),
          ),
        ],
      ),
    );
  }
}

class _CompactCounter extends StatelessWidget {
  const _CompactCounter({
    required this.workspace,
    required this.visibleProducts,
    required this.selectedCategoryId,
    required this.searchQuery,
    required this.searchController,
    required this.searchFocusNode,
    required this.cartItemCount,
    required this.subtotalMinor,
    required this.currencyCode,
    required this.canAddProducts,
    required this.onSearchChanged,
    required this.onCategoryChanged,
    required this.onAdd,
    required this.openDraftCount,
    this.onOpenDrafts,
    required this.onViewCart,
  });

  final CommunityWorkspace workspace;
  final List<CommunityProductView> visibleProducts;
  final String? selectedCategoryId;
  final String searchQuery;
  final TextEditingController searchController;
  final FocusNode searchFocusNode;
  final int cartItemCount;
  final int subtotalMinor;
  final String currencyCode;
  final bool canAddProducts;
  final ValueChanged<String> onSearchChanged;
  final ValueChanged<String?> onCategoryChanged;
  final ValueChanged<CommunityProductView> onAdd;
  final int openDraftCount;
  final VoidCallback? onOpenDrafts;
  final VoidCallback onViewCart;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Expanded(
          child: _CatalogPane(
            workspace: workspace,
            visibleProducts: visibleProducts,
            selectedCategoryId: selectedCategoryId,
            searchQuery: searchQuery,
            searchController: searchController,
            searchFocusNode: searchFocusNode,
            onSearchChanged: onSearchChanged,
            onCategoryChanged: onCategoryChanged,
            onAdd: onAdd,
            compact: true,
            canAddProducts: canAddProducts,
          ),
        ),
        SafeArea(
          top: false,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (openDraftCount > 0) ...[
                  FilledButton.tonalIcon(
                    key: const Key('pos-open-drafts'),
                    onPressed: onOpenDrafts,
                    style: FilledButton.styleFrom(
                      minimumSize: const Size.fromHeight(46),
                    ),
                    icon: const Icon(Icons.pending_actions_outlined),
                    label: Text('Open & sent orders ($openDraftCount)'),
                  ),
                  const SizedBox(height: 8),
                ],
                FilledButton(
                  key: const Key('pos-view-order'),
                  onPressed: onViewCart,
                  style: FilledButton.styleFrom(
                    minimumSize: const Size.fromHeight(58),
                    alignment: Alignment.centerLeft,
                  ),
                  child: Row(
                    children: [
                      const Icon(Icons.shopping_bag_outlined),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Text(
                          cartItemCount == 0
                              ? 'Current order is empty'
                              : 'Current order • $cartItemCount item${cartItemCount == 1 ? '' : 's'}',
                        ),
                      ),
                      Flexible(
                        child: Text(
                          formatMinorPrice(subtotalMinor, currencyCode),
                          textAlign: TextAlign.end,
                          style: const TextStyle(fontWeight: FontWeight.w800),
                        ),
                      ),
                      const SizedBox(width: 6),
                      const Icon(Icons.arrow_forward),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _CompactOrderScreen extends StatelessWidget {
  const _CompactOrderScreen({
    required this.cartEntries,
    required this.cartItemCount,
    required this.subtotalMinor,
    required this.currencyCode,
    required this.fulfillment,
    required this.paymentMethod,
    this.kitchenNote,
    required this.customers,
    required this.selectedCustomerId,
    required this.isBusy,
    required this.checkoutStatus,
    required this.receipt,
    required this.draftNeedsSaving,
    required this.onBack,
    required this.onQuantityChanged,
    required this.onFulfillmentChanged,
    required this.onPaymentMethodChanged,
    required this.onCustomerChanged,
    this.onAddCustomer,
    required this.onCheckout,
    required this.onSaveDraft,
    this.onEditKitchenNote,
    required this.isKitchenSent,
    required this.canManageOrders,
    this.onSendToKitchen,
    this.onCancelSentDraft,
    this.onReopenSentDraft,
  });

  final List<_CartEntry> cartEntries;
  final int cartItemCount;
  final int subtotalMinor;
  final String currencyCode;
  final String fulfillment;
  final String paymentMethod;
  final String? kitchenNote;
  final List<CommunityCustomerView> customers;
  final String? selectedCustomerId;
  final bool isBusy;
  final String? checkoutStatus;
  final PosCheckoutResult? receipt;
  final bool draftNeedsSaving;
  final VoidCallback onBack;
  final void Function(_CartEntry entry, int delta) onQuantityChanged;
  final ValueChanged<String> onFulfillmentChanged;
  final ValueChanged<String> onPaymentMethodChanged;
  final ValueChanged<String?> onCustomerChanged;
  final VoidCallback? onAddCustomer;
  final Future<void> Function() onCheckout;
  final Future<void> Function() onSaveDraft;
  final VoidCallback? onEditKitchenNote;
  final bool isKitchenSent;
  final bool canManageOrders;
  final Future<void> Function()? onSendToKitchen;
  final Future<void> Function()? onCancelSentDraft;
  final Future<void> Function()? onReopenSentDraft;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
      child: Column(
        children: [
          Row(
            children: [
              IconButton(
                tooltip: 'Back to menu',
                onPressed: onBack,
                icon: const Icon(Icons.arrow_back),
              ),
              const SizedBox(width: 4),
              Expanded(
                child: Text(
                  'Current order',
                  style: Theme.of(
                    context,
                  ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Expanded(
            child: _CartPanel(
              compact: true,
              cartEntries: cartEntries,
              cartItemCount: cartItemCount,
              subtotalMinor: subtotalMinor,
              currencyCode: currencyCode,
              fulfillment: fulfillment,
              paymentMethod: paymentMethod,
              kitchenNote: kitchenNote,
              customers: customers,
              selectedCustomerId: selectedCustomerId,
              isBusy: isBusy,
              checkoutStatus: checkoutStatus,
              receipt: receipt,
              draftNeedsSaving: draftNeedsSaving,
              onQuantityChanged: onQuantityChanged,
              onFulfillmentChanged: onFulfillmentChanged,
              onPaymentMethodChanged: onPaymentMethodChanged,
              onCustomerChanged: onCustomerChanged,
              onAddCustomer: onAddCustomer,
              onCheckout: onCheckout,
              onSaveDraft: onSaveDraft,
              onEditKitchenNote: onEditKitchenNote,
              isKitchenSent: isKitchenSent,
              canManageOrders: canManageOrders,
              onSendToKitchen: onSendToKitchen,
              onCancelSentDraft: onCancelSentDraft,
              onReopenSentDraft: onReopenSentDraft,
            ),
          ),
        ],
      ),
    );
  }
}

class _CatalogPane extends StatelessWidget {
  const _CatalogPane({
    required this.workspace,
    required this.visibleProducts,
    required this.selectedCategoryId,
    required this.searchQuery,
    required this.searchController,
    required this.searchFocusNode,
    required this.onSearchChanged,
    required this.onCategoryChanged,
    required this.onAdd,
    this.canAddProducts = true,
    this.compact = false,
  });

  final CommunityWorkspace workspace;
  final List<CommunityProductView> visibleProducts;
  final String? selectedCategoryId;
  final String searchQuery;
  final TextEditingController searchController;
  final FocusNode searchFocusNode;
  final ValueChanged<String> onSearchChanged;
  final ValueChanged<String?> onCategoryChanged;
  final ValueChanged<CommunityProductView> onAdd;
  final bool canAddProducts;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final horizontalPadding = compact ? 20.0 : 0.0;
    final scaledBodyFont = MediaQuery.textScalerOf(context).scale(14);
    final productTileExtent = (166 + (scaledBodyFont - 14) * 4)
        .clamp(166.0, 310.0)
        .toDouble();
    // Fewer, wider columns keep prices and names readable instead of forcing
    // scaled text into narrow cards. At the default scale this preserves the
    // dense counter grid; at 200% a phone uses one comfortable column.
    final productMaxCrossAxisExtent = (260 + (scaledBodyFont - 14) * 18)
        .clamp(260.0, 520.0)
        .toDouble();
    return CustomScrollView(
      key: const PageStorageKey('pos-catalog-scroll'),
      slivers: [
        SliverPadding(
          padding: EdgeInsets.fromLTRB(
            horizontalPadding,
            0,
            horizontalPadding,
            28,
          ),
          sliver: SliverList(
            delegate: SliverChildListDelegate([
              Text(
                'Counter',
                style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                  fontWeight: FontWeight.w800,
                  letterSpacing: -0.7,
                ),
              ),
              const SizedBox(height: 6),
              Text(
                '${workspace.branchName ?? 'Your branch'} • works from local truth',
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: 20),
              TextField(
                key: const Key('pos-search'),
                controller: searchController,
                focusNode: searchFocusNode,
                onChanged: onSearchChanged,
                decoration: InputDecoration(
                  labelText: 'Find a menu item',
                  prefixIcon: const Icon(Icons.search),
                  helperText: 'Keyboard shortcut: Ctrl+F',
                  suffixIcon: searchQuery.isEmpty
                      ? null
                      : IconButton(
                          tooltip: 'Clear menu search',
                          onPressed: () {
                            searchController.clear();
                            onSearchChanged('');
                            searchFocusNode.requestFocus();
                          },
                          icon: const Icon(Icons.clear),
                        ),
                ),
              ),
              const SizedBox(height: 14),
              _CategoryFilters(
                categories: workspace.categories,
                selectedCategoryId: selectedCategoryId,
                onChanged: onCategoryChanged,
              ),
              const SizedBox(height: 22),
              Text(
                visibleProducts.isEmpty ? 'No matching menu items' : 'Menu',
                style: Theme.of(
                  context,
                ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
              ),
              const SizedBox(height: 12),
            ]),
          ),
        ),
        if (visibleProducts.isEmpty)
          SliverPadding(
            padding: EdgeInsets.symmetric(horizontal: horizontalPadding),
            sliver: const SliverToBoxAdapter(child: _NoMatchingProducts()),
          )
        else
          SliverPadding(
            padding: EdgeInsets.fromLTRB(
              horizontalPadding,
              0,
              horizontalPadding,
              28,
            ),
            sliver: SliverGrid(
              delegate: SliverChildBuilderDelegate(
                (context, index) => _ProductTile(
                  product: visibleProducts[index],
                  onAdd: () => onAdd(visibleProducts[index]),
                  enabled: canAddProducts,
                ),
                childCount: visibleProducts.length,
              ),
              gridDelegate: SliverGridDelegateWithMaxCrossAxisExtent(
                maxCrossAxisExtent: productMaxCrossAxisExtent,
                mainAxisExtent: productTileExtent,
                mainAxisSpacing: 12,
                crossAxisSpacing: 12,
              ),
            ),
          ),
      ],
    );
  }
}

class _CategoryFilters extends StatelessWidget {
  const _CategoryFilters({
    required this.categories,
    required this.selectedCategoryId,
    required this.onChanged,
  });

  final List<CommunityCategoryView> categories;
  final String? selectedCategoryId;
  final ValueChanged<String?> onChanged;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: [
          ChoiceChip(
            label: const Text('All items'),
            selected: selectedCategoryId == null,
            onSelected: (_) => onChanged(null),
          ),
          for (final category in categories) ...[
            const SizedBox(width: 8),
            ChoiceChip(
              label: Text(category.displayName),
              selected: selectedCategoryId == category.categoryId,
              onSelected: (_) => onChanged(category.categoryId),
            ),
          ],
        ],
      ),
    );
  }
}

class _ProductTile extends StatelessWidget {
  const _ProductTile({
    required this.product,
    required this.onAdd,
    required this.enabled,
  });

  final CommunityProductView product;
  final VoidCallback onAdd;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final price = formatMinorPrice(
      product.unitPriceMinor,
      product.currencyCode,
    );
    final hasModifiers = product.modifierOptions.any(
      (option) => !option.archived,
    );
    final actionLabel = enabled
        ? '${hasModifiers ? 'Customise' : 'Add'} ${product.displayName}, $price'
        : '${product.displayName}, $price, unavailable';

    // A product card used to expose both the card and its trailing plus icon
    // as separate, ambiguously labelled actions. The card is one action in
    // the counter workflow, so give assistive technology one descriptive
    // control and keep a single keyboard focus stop for it.
    return Semantics(
      button: true,
      enabled: enabled,
      label: actionLabel,
      hint: enabled
          ? hasModifiers
                ? 'Choose optional additions, then add one item to the current order.'
                : 'Adds one item to the current order.'
          : 'This menu item cannot be added to the current order.',
      onTap: enabled ? onAdd : null,
      excludeSemantics: true,
      child: Card(
        key: ValueKey('pos-product-${product.productId}'),
        clipBehavior: Clip.antiAlias,
        child: Tooltip(
          message: actionLabel,
          child: InkWell(
            key: ValueKey('pos-add-${product.productId}'),
            onTap: enabled ? onAdd : null,
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  SizedBox(
                    height: 52,
                    width: 52,
                    child: MenuItemImage(
                      assetKey: product.imageAssetKey,
                      imageBytes: product.imageBytes,
                      borderRadius: const BorderRadius.all(Radius.circular(12)),
                      cacheWidth: 104,
                      cacheHeight: 104,
                    ),
                  ),
                  const Spacer(),
                  Text(
                    product.displayName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  const SizedBox(height: 4),
                  if (hasModifiers)
                    Text(
                      'Customisable',
                      style: Theme.of(context).textTheme.labelSmall?.copyWith(
                        color: Theme.of(context).colorScheme.primary,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  if (hasModifiers) const SizedBox(height: 2),
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          price,
                          style: Theme.of(context).textTheme.bodyMedium
                              ?.copyWith(
                                color: Theme.of(
                                  context,
                                ).colorScheme.onSurfaceVariant,
                                fontWeight: FontWeight.w700,
                              ),
                        ),
                      ),
                      DecoratedBox(
                        decoration: ShapeDecoration(
                          color: Theme.of(
                            context,
                          ).colorScheme.secondaryContainer,
                          shape: const RoundedRectangleBorder(
                            borderRadius: BorderRadius.all(Radius.circular(12)),
                          ),
                        ),
                        child: SizedBox(
                          height: 40,
                          width: 40,
                          child: Icon(
                            Icons.add,
                            color: Theme.of(
                              context,
                            ).colorScheme.onSecondaryContainer,
                          ),
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _NoMatchingProducts extends StatelessWidget {
  const _NoMatchingProducts();

  @override
  Widget build(BuildContext context) {
    return Card(
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      child: const Padding(
        padding: EdgeInsets.all(22),
        child: Row(
          children: [
            Icon(Icons.search_off_outlined),
            SizedBox(width: 12),
            Expanded(
              child: Text('Try another name or choose a different category.'),
            ),
          ],
        ),
      ),
    );
  }
}

class _CartPanel extends StatelessWidget {
  const _CartPanel({
    required this.compact,
    required this.cartEntries,
    required this.cartItemCount,
    required this.subtotalMinor,
    required this.currencyCode,
    required this.fulfillment,
    required this.paymentMethod,
    this.kitchenNote,
    required this.customers,
    required this.selectedCustomerId,
    required this.isBusy,
    required this.checkoutStatus,
    required this.receipt,
    required this.draftNeedsSaving,
    required this.onQuantityChanged,
    required this.onFulfillmentChanged,
    required this.onPaymentMethodChanged,
    required this.onCustomerChanged,
    this.onAddCustomer,
    required this.onCheckout,
    required this.onSaveDraft,
    this.onEditKitchenNote,
    required this.isKitchenSent,
    required this.canManageOrders,
    this.onSendToKitchen,
    this.onCancelSentDraft,
    this.onReopenSentDraft,
  });

  final bool compact;
  final List<_CartEntry> cartEntries;
  final int cartItemCount;
  final int subtotalMinor;
  final String currencyCode;
  final String fulfillment;
  final String paymentMethod;
  final String? kitchenNote;
  final List<CommunityCustomerView> customers;
  final String? selectedCustomerId;
  final bool isBusy;
  final String? checkoutStatus;
  final PosCheckoutResult? receipt;
  final bool draftNeedsSaving;
  final void Function(_CartEntry entry, int delta) onQuantityChanged;
  final ValueChanged<String> onFulfillmentChanged;
  final ValueChanged<String> onPaymentMethodChanged;
  final ValueChanged<String?> onCustomerChanged;
  final VoidCallback? onAddCustomer;
  final Future<void> Function() onCheckout;
  final Future<void> Function() onSaveDraft;
  final VoidCallback? onEditKitchenNote;
  final bool isKitchenSent;
  final bool canManageOrders;
  final Future<void> Function()? onSendToKitchen;
  final Future<void> Function()? onCancelSentDraft;
  final Future<void> Function()? onReopenSentDraft;

  @override
  Widget build(BuildContext context) {
    final hasCart = cartEntries.isNotEmpty;
    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                'Current order',
                style: Theme.of(
                  context,
                ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
              ),
            ),
            if (cartItemCount > 0)
              Badge(
                label: Text('$cartItemCount'),
                child: const Icon(Icons.shopping_bag_outlined),
              )
            else
              const Icon(Icons.shopping_bag_outlined),
          ],
        ),
        const SizedBox(height: 4),
        Text(
          isKitchenSent
              ? 'Kitchen-sent items and service are locked. Payment can be recorded when ready.'
              : 'Unsaved cart changes can be removed. Recorded sales stay in the audit history.',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
        ),
        if (isKitchenSent) ...[
          const SizedBox(height: 12),
          _KitchenSentOrderNotice(
            canManageOrders: canManageOrders,
            isBusy: isBusy,
            onReopen: onReopenSentDraft,
            onCancel: onCancelSentDraft,
          ),
        ],
        if (kitchenNote != null) ...[
          const SizedBox(height: 12),
          _KitchenInstructionSummary(note: kitchenNote!, locked: isKitchenSent),
        ],
        if (!isKitchenSent && onEditKitchenNote != null) ...[
          const SizedBox(height: 12),
          OutlinedButton.icon(
            key: const Key('pos-edit-kitchen-note'),
            onPressed: isBusy ? null : onEditKitchenNote,
            icon: Icon(
              kitchenNote == null
                  ? Icons.note_add_outlined
                  : Icons.edit_note_outlined,
            ),
            label: Text(
              kitchenNote == null
                  ? 'Add kitchen instruction'
                  : 'Edit kitchen instruction',
            ),
          ),
        ],
        const SizedBox(height: 16),
        if (hasCart)
          Column(
            key: const PageStorageKey('pos-cart-items'),
            children: [
              for (var index = 0; index < cartEntries.length; index++) ...[
                if (index > 0) const Divider(height: 1),
                _CartLine(
                  entry: cartEntries[index],
                  isBusy: isBusy || isKitchenSent,
                  onSubtract: () => onQuantityChanged(cartEntries[index], -1),
                  onAdd: () => onQuantityChanged(cartEntries[index], 1),
                ),
              ],
            ],
          )
        else
          const SizedBox(height: 132, child: _EmptyCart()),
        const SizedBox(height: 14),
        _SelectorLabel(label: 'Service'),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            _SelectionChip(
              label: 'Takeaway',
              icon: Icons.takeout_dining_outlined,
              selected: fulfillment == 'takeaway',
              enabled: !isBusy && !isKitchenSent,
              onSelected: () => onFulfillmentChanged('takeaway'),
            ),
            _SelectionChip(
              label: 'Dine in',
              icon: Icons.table_restaurant_outlined,
              selected: fulfillment == 'dine_in',
              enabled: !isBusy && !isKitchenSent,
              onSelected: () => onFulfillmentChanged('dine_in'),
            ),
          ],
        ),
        const SizedBox(height: 14),
        _SelectorLabel(label: 'Payment received by'),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            _SelectionChip(
              label: 'Cash',
              icon: Icons.payments_outlined,
              selected: paymentMethod == 'cash',
              enabled: !isBusy,
              onSelected: () => onPaymentMethodChanged('cash'),
            ),
            _SelectionChip(
              label: 'Card',
              icon: Icons.credit_card_outlined,
              selected: paymentMethod == 'card',
              enabled: !isBusy,
              onSelected: () => onPaymentMethodChanged('card'),
            ),
            _SelectionChip(
              label: 'UPI',
              icon: Icons.qr_code_2_outlined,
              selected: paymentMethod == 'upi',
              enabled: !isBusy,
              onSelected: () => onPaymentMethodChanged('upi'),
            ),
            _SelectionChip(
              label: 'Split',
              icon: Icons.call_split_outlined,
              selected: paymentMethod == 'split',
              enabled: !isBusy,
              onSelected: () => onPaymentMethodChanged('split'),
            ),
          ],
        ),
        const SizedBox(height: 14),
        _SelectorLabel(label: 'Customer (optional)'),
        const SizedBox(height: 8),
        DropdownButtonFormField<String>(
          initialValue: selectedCustomerId,
          isExpanded: true,
          decoration: const InputDecoration(
            border: OutlineInputBorder(),
            prefixIcon: Icon(Icons.person_outline),
            hintText: 'No customer attached',
          ),
          hint: const Text('No customer attached'),
          items: customers
              .map(
                (customer) => DropdownMenuItem<String>(
                  value: customer.customerId,
                  child: Text(
                    customer.displayName,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              )
              .toList(growable: false),
          onChanged: isBusy ? null : onCustomerChanged,
        ),
        if (selectedCustomerId != null)
          Align(
            alignment: Alignment.centerRight,
            child: TextButton.icon(
              onPressed: isBusy ? null : () => onCustomerChanged(null),
              icon: const Icon(Icons.person_remove_outlined),
              label: const Text('Remove customer'),
            ),
          ),
        if (onAddCustomer != null)
          Align(
            alignment: Alignment.centerRight,
            child: TextButton.icon(
              onPressed: isBusy ? null : onAddCustomer,
              icon: const Icon(Icons.person_add_alt_1_outlined),
              label: const Text('Add customer'),
            ),
          ),
        const SizedBox(height: 16),
        _OrderTotal(subtotalMinor: subtotalMinor, currencyCode: currencyCode),
        const SizedBox(height: 6),
        Text(
          'Catalogue subtotal shown here. Payable tax and discounts are calculated by Rust at checkout before split tender or payment.',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
        ),
        if (checkoutStatus != null) ...[
          const SizedBox(height: 12),
          _CheckoutStatus(
            status: checkoutStatus!,
            successful: receipt?.recorded ?? false,
          ),
        ],
        if (receipt?.recorded ?? false) ...[
          const SizedBox(height: 10),
          _SavedReceiptSummary(receipt: receipt!),
        ],
        if (onSendToKitchen != null) ...[
          const SizedBox(height: 14),
          FilledButton.tonalIcon(
            key: const Key('pos-send-to-kitchen'),
            onPressed: hasCart && !isBusy
                ? (draftNeedsSaving ? onSaveDraft : onSendToKitchen)
                : null,
            style: FilledButton.styleFrom(
              minimumSize: const Size.fromHeight(48),
            ),
            icon: Icon(
              draftNeedsSaving
                  ? Icons.save_outlined
                  : Icons.soup_kitchen_outlined,
            ),
            label: Text(
              draftNeedsSaving
                  ? 'Save changes before sending'
                  : 'Send to kitchen',
            ),
          ),
        ],
        const SizedBox(height: 14),
        FilledButton.icon(
          key: const Key('pos-save-draft'),
          onPressed: hasCart && !isBusy && !isKitchenSent ? onSaveDraft : null,
          style: FilledButton.styleFrom(minimumSize: const Size.fromHeight(48)),
          icon: const Icon(Icons.pause_circle_outline),
          label: const Text('Hold open order'),
        ),
        const SizedBox(height: 8),
        FilledButton.icon(
          key: const Key('pos-checkout'),
          onPressed: hasCart && !isBusy ? onCheckout : null,
          style: FilledButton.styleFrom(minimumSize: const Size.fromHeight(52)),
          icon: isBusy
              ? const SizedBox(
                  height: 18,
                  width: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.lock_outline),
          label: Text(
            isBusy
                ? 'Saving sale locally…'
                : 'Record ${_paymentLabel(paymentMethod)} sale',
          ),
        ),
      ],
    );

    // A desktop counter may be short (or use a large accessibility text
    // scale), so the complete order panel must be scrollable. Keeping only
    // its line list scrollable made the service, payment, and action controls
    // overflow at the bottom of a short window.
    return Card(
      child: SingleChildScrollView(
        key: PageStorageKey<String>(
          compact ? 'pos-order-scroll' : 'pos-desktop-order-scroll',
        ),
        padding: const EdgeInsets.all(20),
        child: content,
      ),
    );
  }
}

class _KitchenInstructionSummary extends StatelessWidget {
  const _KitchenInstructionSummary({required this.note, required this.locked});

  final String note;
  final bool locked;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return Semantics(
      label: '${locked ? 'Locked' : 'Pending'} kitchen instruction: $note',
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: colorScheme.tertiaryContainer,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(
                locked ? Icons.lock_outline : Icons.restaurant_outlined,
                color: colorScheme.onTertiaryContainer,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      locked
                          ? 'Kitchen instruction (locked)'
                          : 'Kitchen instruction',
                      style: TextStyle(
                        color: colorScheme.onTertiaryContainer,
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      note,
                      maxLines: 5,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(color: colorScheme.onTertiaryContainer),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _KitchenSentOrderNotice extends StatelessWidget {
  const _KitchenSentOrderNotice({
    required this.canManageOrders,
    required this.isBusy,
    this.onReopen,
    this.onCancel,
  });

  final bool canManageOrders;
  final bool isBusy;
  final Future<void> Function()? onReopen;
  final Future<void> Function()? onCancel;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final hasManagementAction = onReopen != null || onCancel != null;
    return Semantics(
      liveRegion: true,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: colorScheme.errorContainer,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    Icons.lock_clock_outlined,
                    color: colorScheme.onErrorContainer,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Sent to kitchen — items and service are locked.',
                      style: TextStyle(
                        color: colorScheme.onErrorContainer,
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 6),
              Text(
                canManageOrders
                    ? 'Reopen creates a new retained revision; cancel alerts the kitchen and keeps the full history.'
                    : 'Only a manager or owner can reopen or cancel this kitchen-sent order.',
                style: TextStyle(color: colorScheme.onErrorContainer),
              ),
              if (canManageOrders && hasManagementAction) ...[
                const SizedBox(height: 12),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    if (onReopen != null)
                      OutlinedButton.icon(
                        onPressed: isBusy ? null : onReopen,
                        icon: const Icon(Icons.restart_alt_outlined),
                        label: const Text('Reopen new revision'),
                      ),
                    if (onCancel != null)
                      FilledButton.tonalIcon(
                        onPressed: isBusy ? null : onCancel,
                        icon: const Icon(Icons.cancel_outlined),
                        label: const Text('Cancel sent order'),
                      ),
                  ],
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _SavedReceiptSummary extends StatelessWidget {
  const _SavedReceiptSummary({required this.receipt});

  final PosCheckoutResult receipt;

  @override
  Widget build(BuildContext context) {
    final invoice = receipt.invoiceNumber ?? 'recorded';
    final total = receipt.totalMinor == null || receipt.currencyCode == null
        ? null
        : formatMinorPrice(receipt.totalMinor!, receipt.currencyCode!);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.primaryContainer,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Row(
          children: [
            Icon(
              Icons.verified_outlined,
              color: Theme.of(context).colorScheme.onPrimaryContainer,
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                'Receipt $invoice • ${receipt.paymentMethod?.toUpperCase() ?? 'PAYMENT RECORDED'}',
                style: TextStyle(
                  color: Theme.of(context).colorScheme.onPrimaryContainer,
                  fontWeight: FontWeight.w800,
                ),
              ),
            ),
            if (total != null) ...[
              const SizedBox(width: 8),
              Flexible(
                child: Text(
                  total,
                  textAlign: TextAlign.end,
                  style: TextStyle(
                    color: Theme.of(context).colorScheme.onPrimaryContainer,
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _CartLine extends StatelessWidget {
  const _CartLine({
    required this.entry,
    required this.isBusy,
    required this.onSubtract,
    required this.onAdd,
  });

  final _CartEntry entry;
  final bool isBusy;
  final VoidCallback onSubtract;
  final VoidCallback onAdd;

  @override
  Widget build(BuildContext context) {
    final price = formatMinorPrice(
      entry.lineTotalMinor,
      entry.product.currencyCode,
    );
    final modifiers = entry.modifierOptions
        .map((option) => option.displayName)
        .join(', ');
    final lineLabel = [
      entry.product.displayName,
      if (modifiers.isNotEmpty) 'with $modifiers',
      'quantity ${entry.quantity}',
      'line total $price',
    ].join(', ');
    return Padding(
      key: ValueKey('pos-cart-${entry.key.stableKey}'),
      padding: const EdgeInsets.symmetric(vertical: 12),
      child: Row(
        children: [
          Expanded(
            child: Semantics(
              label: lineLabel,
              excludeSemantics: true,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    entry.product.displayName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontWeight: FontWeight.w800),
                  ),
                  if (entry.modifierOptions.isNotEmpty) ...[
                    const SizedBox(height: 3),
                    Text(
                      entry.modifierOptions
                          .map((option) => option.displayName)
                          .join(' • '),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: Theme.of(context).colorScheme.primary,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ],
                  const SizedBox(height: 3),
                  Text(
                    price,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
          ),
          IconButton(
            key: ValueKey('pos-remove-one-${entry.key.stableKey}'),
            tooltip: 'Remove one ${entry.product.displayName}',
            onPressed: isBusy ? null : onSubtract,
            icon: const Icon(Icons.remove_circle_outline),
          ),
          ExcludeSemantics(
            child: Text(
              '${entry.quantity}',
              style: const TextStyle(fontWeight: FontWeight.w800),
            ),
          ),
          IconButton(
            key: ValueKey('pos-add-one-${entry.key.stableKey}'),
            tooltip: 'Add one ${entry.product.displayName}',
            onPressed: isBusy ? null : onAdd,
            icon: const Icon(Icons.add_circle_outline),
          ),
        ],
      ),
    );
  }
}

class _EmptyCart extends StatelessWidget {
  const _EmptyCart();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.add_shopping_cart_outlined,
            size: 34,
            color: Theme.of(context).colorScheme.primary,
          ),
          const SizedBox(height: 10),
          Text(
            'Add items to start an order.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }
}

class _SelectorLabel extends StatelessWidget {
  const _SelectorLabel({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Text(
      label,
      style: Theme.of(
        context,
      ).textTheme.labelLarge?.copyWith(fontWeight: FontWeight.w800),
    );
  }
}

class _SelectionChip extends StatelessWidget {
  const _SelectionChip({
    required this.label,
    required this.icon,
    required this.selected,
    required this.enabled,
    required this.onSelected,
  });

  final String label;
  final IconData icon;
  final bool selected;
  final bool enabled;
  final VoidCallback onSelected;

  @override
  Widget build(BuildContext context) {
    return ChoiceChip(
      avatar: Icon(icon, size: 17),
      label: Text(label),
      selected: selected,
      onSelected: enabled ? (_) => onSelected() : null,
    );
  }
}

class _OrderTotal extends StatelessWidget {
  const _OrderTotal({required this.subtotalMinor, required this.currencyCode});

  final int subtotalMinor;
  final String currencyCode;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: Text(
            'Subtotal',
            style: Theme.of(
              context,
            ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700),
          ),
        ),
        Flexible(
          child: Text(
            formatMinorPrice(subtotalMinor, currencyCode),
            textAlign: TextAlign.end,
            style: Theme.of(
              context,
            ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w900),
          ),
        ),
      ],
    );
  }
}

class _CheckoutStatus extends StatelessWidget {
  const _CheckoutStatus({required this.status, required this.successful});

  final String status;
  final bool successful;

  @override
  Widget build(BuildContext context) {
    final color = successful
        ? Theme.of(context).colorScheme.primary
        : Theme.of(context).colorScheme.error;
    return Semantics(
      liveRegion: true,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.1),
          borderRadius: const BorderRadius.all(Radius.circular(12)),
        ),
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(
                successful ? Icons.verified_outlined : Icons.error_outline,
                color: color,
              ),
              const SizedBox(width: 8),
              Expanded(child: Text(status)),
            ],
          ),
        ),
      ),
    );
  }
}

class _PosUnavailable extends StatelessWidget {
  const _PosUnavailable({
    required this.icon,
    required this.title,
    required this.detail,
    this.actionLabel,
    this.onAction,
  });

  final IconData icon;
  final String title;
  final String detail;
  final String? actionLabel;
  final VoidCallback? onAction;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Padding(
          padding: const EdgeInsets.all(28),
          child: Card(
            child: Padding(
              padding: const EdgeInsets.all(28),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    icon,
                    color: Theme.of(context).colorScheme.primary,
                    size: 34,
                  ),
                  const SizedBox(height: 16),
                  Text(
                    title,
                    style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  const SizedBox(height: 10),
                  Text(
                    detail,
                    style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  ),
                  if (onAction != null && actionLabel != null) ...[
                    const SizedBox(height: 22),
                    FilledButton.icon(
                      onPressed: onAction,
                      icon: const Icon(Icons.menu_book_outlined),
                      label: Text(actionLabel!),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _CartLineKey {
  _CartLineKey(this.productId, Iterable<String> modifierOptionIds)
    : modifierOptionIds = List.unmodifiable(
        (List<String>.from(modifierOptionIds)..sort()),
      );

  final String productId;
  final List<String> modifierOptionIds;

  String get stableKey => modifierOptionIds.isEmpty
      ? productId
      : '$productId:${modifierOptionIds.join(',')}';

  @override
  int get hashCode => Object.hash(productId, Object.hashAll(modifierOptionIds));

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is _CartLineKey &&
          productId == other.productId &&
          _sameStringList(modifierOptionIds, other.modifierOptionIds);
}

class _CartEntry {
  const _CartEntry({
    required this.key,
    required this.product,
    required this.quantity,
    required this.modifierOptions,
  });

  final _CartLineKey key;
  final CommunityProductView product;
  final int quantity;
  final List<CommunityModifierOptionView> modifierOptions;

  int get modifierTotalMinor => modifierOptions.fold(
    0,
    (total, option) => total + option.priceDeltaMinor,
  );

  int get unitPriceMinor => product.unitPriceMinor + modifierTotalMinor;

  int get lineTotalMinor => unitPriceMinor * quantity;
}

String _paymentLabel(String paymentMethod) {
  return switch (paymentMethod) {
    'cash' => 'cash',
    'card' => 'card',
    'upi' => 'UPI',
    _ => 'payment',
  };
}
