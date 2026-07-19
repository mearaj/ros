import 'dart:async';
import 'dart:convert';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'branch_time.dart';
import 'diagnostics_breadcrumbs.dart';
import 'diagnostics_share.dart';
import 'money_input.dart';
import '../catalog/menu_item_image.dart';
import '../catalog/remote_menu_image_catalog.dart';
import '../point_of_sale/pos_workspace.dart';
import '../../src/rust/api/simple.dart';
import '../../theme/app_theme.dart';

enum _Destination { overview, pointOfSale, kitchen, inventory, reports }

class RestaurantShell extends StatefulWidget {
  const RestaurantShell({
    required this.coreStatus,
    required this.workspace,
    required this.applicationSupportDirectory,
    this.staffSecurity,
    super.key,
  });

  final String coreStatus;
  final CommunityWorkspace workspace;
  final String applicationSupportDirectory;
  final CommunityStaffSecurity? staffSecurity;

  @override
  State<RestaurantShell> createState() => _RestaurantShellState();
}

class _RestaurantShellState extends State<RestaurantShell> {
  _Destination _destination = _Destination.overview;
  late CommunityWorkspace _workspace;
  CommunityStaffSecurity? _staffSecurity;
  Timer? _sessionRefreshTimer;
  var _isSaving = false;

  @override
  void initState() {
    super.initState();
    _workspace = widget.workspace;
    _staffSecurity = widget.staffSecurity;
    _sessionRefreshTimer = Timer.periodic(const Duration(seconds: 15), (_) {
      if (mounted &&
          !_workspace.setupRequired &&
          _staffSecurity?.activeStaffId != null) {
        unawaited(_refreshStaffSecurity());
      }
    });
  }

  @override
  void dispose() {
    _sessionRefreshTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_requiresStaffUnlock) {
      return _StaffSecurityGate(
        security: _staffSecurity ?? _unavailableStaffSecurity,
        isSaving: _isSaving,
        onConfigureOwnerPin: _configureOwnerPin,
        onUnlock: _unlockStaff,
        onRetry: _refreshStaffSecurity,
      );
    }
    return LayoutBuilder(
      builder: (context, constraints) {
        final textScale = MediaQuery.textScalerOf(context).scale(1);
        // A NavigationRail needs enough vertical room for five destinations,
        // the brand, and an accessible text scale. Falling back to the bottom
        // navigation is preferable to clipping navigation actions when a
        // desktop window is short, a keyboard is visible, or large text is in
        // use.
        final isWide =
            constraints.maxWidth >= 920 &&
            constraints.maxHeight >= 680 &&
            textScale < 1.4;

        return Scaffold(
          body: Row(
            children: [
              if (isWide)
                _Sidebar(
                  destination: _destination,
                  onChanged: _setDestination,
                  showLocalOnlyBadge: constraints.maxHeight >= 760,
                ),
              Expanded(
                child: SafeArea(
                  child: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 180),
                    child: _buildContent(),
                  ),
                ),
              ),
            ],
          ),
          bottomNavigationBar: isWide
              ? null
              : NavigationBar(
                  selectedIndex: _Destination.values.indexOf(_destination),
                  onDestinationSelected: (index) {
                    _setDestination(_Destination.values[index]);
                  },
                  destinations: const [
                    NavigationDestination(
                      icon: Icon(Icons.space_dashboard_outlined),
                      selectedIcon: Icon(Icons.space_dashboard),
                      label: 'Overview',
                    ),
                    NavigationDestination(
                      icon: Icon(Icons.point_of_sale_outlined),
                      selectedIcon: Icon(Icons.point_of_sale),
                      label: 'POS',
                    ),
                    NavigationDestination(
                      icon: Icon(Icons.soup_kitchen_outlined),
                      selectedIcon: Icon(Icons.soup_kitchen),
                      label: 'Kitchen',
                    ),
                    NavigationDestination(
                      icon: Icon(Icons.menu_book_outlined),
                      selectedIcon: Icon(Icons.menu_book),
                      label: 'Menu',
                    ),
                    NavigationDestination(
                      icon: Icon(Icons.insights_outlined),
                      selectedIcon: Icon(Icons.insights),
                      label: 'Reports',
                    ),
                  ],
                ),
          floatingActionButton: _staffSecurity?.activeStaffId == null
              ? null
              : FloatingActionButton.small(
                  tooltip: 'Lock staff session',
                  onPressed: _isSaving ? null : _lockStaff,
                  child: const Icon(Icons.lock_outline),
                ),
        );
      },
    );
  }

  void _setDestination(_Destination destination) {
    if (!_canAccessDestination(destination)) {
      final messenger = ScaffoldMessenger.maybeOf(context);
      final message = _storageNeedsAttention
          ? 'Secure local storage must be resolved before opening this workspace.'
          : 'This workspace is not available to the active role.';
      messenger
        ?..clearSnackBars()
        ..showSnackBar(SnackBar(content: Text(message)));
      return;
    }
    final breadcrumb = switch (destination) {
      _Destination.overview => DiagnosticBreadcrumbs.navOverview,
      _Destination.pointOfSale => DiagnosticBreadcrumbs.navPos,
      _Destination.kitchen => DiagnosticBreadcrumbs.navKitchen,
      _Destination.inventory => DiagnosticBreadcrumbs.navInventory,
      _Destination.reports => DiagnosticBreadcrumbs.navReports,
    };
    recordDiagnosticBreadcrumb(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      eventCode: breadcrumb,
    );
    setState(() {
      _destination = destination;
    });
  }

  bool get _storageNeedsAttention =>
      _workspace.storageStatus.startsWith('Local storage needs attention');

  bool _canAccessDestination(_Destination destination) {
    if (_storageNeedsAttention) {
      return destination == _Destination.overview ||
          destination == _Destination.inventory;
    }
    if (_workspace.setupRequired) {
      return destination == _Destination.overview ||
          destination == _Destination.inventory;
    }
    final role = _staffSecurity?.activeStaffRole;
    return switch (destination) {
      _Destination.overview => true,
      _Destination.pointOfSale =>
        role == 'owner' || role == 'manager' || role == 'cashier',
      _Destination.kitchen =>
        role == 'owner' || role == 'manager' || role == 'kitchen',
      _Destination.inventory ||
      _Destination.reports => role == 'owner' || role == 'manager',
    };
  }

  bool get _requiresStaffUnlock {
    final security = _staffSecurity;
    return !_workspace.setupRequired &&
        (security == null ||
            !security.available ||
            security.ownerPinSetupRequired ||
            security.activeStaffId == null);
  }

  static const _unavailableStaffSecurity = CommunityStaffSecurity(
    storageStatus:
        'Staff security needs attention • local authorization state is unavailable',
    available: false,
    ownerPinSetupRequired: false,
    staff: [],
  );

  CommunityWorkspace _redactedWorkspace(String status) => CommunityWorkspace(
    storageStatus: status,
    setupRequired: false,
    categories: const [],
    products: const [],
    customers: const [],
    openDrafts: const [],
    kitchenTickets: const [],
  );

  Future<void> _refreshStaffSecurity({bool force = false}) async {
    if ((!force && _isSaving) || widget.applicationSupportDirectory.isEmpty) {
      return;
    }
    setState(() => _isSaving = true);
    try {
      final security = await loadCommunityStaffSecurity(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      if (mounted) {
        final previousStaffId = _staffSecurity?.activeStaffId;
        final previousRole = _staffSecurity?.activeStaffRole;
        final shouldReloadWorkspace =
            security.activeStaffId != null &&
            (security.activeStaffId != previousStaffId ||
                security.activeStaffRole != previousRole);
        setState(() {
          _staffSecurity = security;
          if (security.activeStaffId == null) {
            _workspace = _redactedWorkspace(security.storageStatus);
          }
        });
        if (shouldReloadWorkspace) {
          await _reloadAuthorizedWorkspace(security.activeStaffId!);
        }
      }
    } finally {
      if (mounted) {
        setState(() => _isSaving = false);
      }
    }
  }

  Future<void> _configureOwnerPin(String pin) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }
    setState(() => _isSaving = true);
    try {
      final security = await configureCommunityOwnerPin(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        pin: pin,
      );
      if (mounted) {
        setState(() => _staffSecurity = security);
        if (security.activeStaffId != null) {
          await _reloadAuthorizedWorkspace(security.activeStaffId!);
        }
      }
    } finally {
      if (mounted) {
        setState(() => _isSaving = false);
      }
    }
  }

  Future<void> _unlockStaff(String staffId, String pin) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }
    setState(() => _isSaving = true);
    try {
      final security = await unlockCommunityStaff(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        staffId: staffId,
        pin: pin,
      );
      if (mounted) {
        setState(() => _staffSecurity = security);
        if (security.activeStaffId != null) {
          await _reloadAuthorizedWorkspace(security.activeStaffId!);
        } else {
          setState(
            () => _workspace = _redactedWorkspace(security.storageStatus),
          );
        }
      }
    } finally {
      if (mounted) {
        setState(() => _isSaving = false);
      }
    }
  }

  Future<void> _lockStaff() async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }
    setState(() => _isSaving = true);
    try {
      final security = await lockCommunityStaff(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      if (mounted) {
        setState(() {
          _staffSecurity = security;
          _workspace = _redactedWorkspace(security.storageStatus);
          _destination = _Destination.overview;
        });
      }
    } finally {
      if (mounted) {
        setState(() => _isSaving = false);
      }
    }
  }

  Future<void> _reloadAuthorizedWorkspace(String expectedStaffId) async {
    if (widget.applicationSupportDirectory.isEmpty) return;
    final workspace = await loadCommunityWorkspace(
      applicationSupportDirectory: widget.applicationSupportDirectory,
    );
    if (!mounted || _staffSecurity?.activeStaffId != expectedStaffId) return;
    setState(() => _workspace = workspace);
  }

  Widget _buildContent() {
    final role = _staffSecurity?.activeStaffRole;
    final canManage =
        _workspace.setupRequired || role == 'owner' || role == 'manager';
    final canCounter =
        _workspace.setupRequired ||
        role == 'owner' ||
        role == 'manager' ||
        role == 'cashier';
    final canKitchen =
        _workspace.setupRequired ||
        role == 'owner' ||
        role == 'manager' ||
        role == 'kitchen';
    if (_storageNeedsAttention &&
        (_destination == _Destination.overview ||
            _destination == _Destination.inventory)) {
      return _StorageAttentionWorkspace(
        key: ValueKey('storage-attention-$_destination'),
        status: _workspace.storageStatus,
        isSaving: _isSaving,
        canStartFresh: _workspace.storageStatus.contains(
          'local data recovery is required',
        ),
        onRetry: _retryLocalStorage,
        onStartFresh: _resetStorageForFreshSetup,
      );
    }
    return switch (_destination) {
      _Destination.overview => _Overview(
        key: const ValueKey('overview'),
        coreStatus: '${widget.coreStatus} • ${_workspace.storageStatus}',
        setupRequired: _workspace.setupRequired,
        branchName: _workspace.branchName,
        openOrderCount: _workspace.openDrafts.length,
        kitchenQueueCount: _workspace.kitchenTickets.length,
        onOpenPos: () => _setDestination(_Destination.pointOfSale),
        onStartSetup: () => _setDestination(_Destination.inventory),
      ),
      _Destination.pointOfSale => PosWorkspace(
        key: const ValueKey('point-of-sale'),
        workspace: _workspace,
        canOperateCounter: canCounter,
        canManageOrders: canManage,
        isSaving: _isSaving,
        onCheckout: _completeCommunitySale,
        onPreviewPricing: _previewCommunitySalePricing,
        onSaveDraft: _saveCommunityDraftOrder,
        onSendToKitchen: _sendCommunityDraftToKitchen,
        onCancelDraft: _cancelCommunityOpenDraft,
        onCancelSentDraft: _cancelCommunitySentDraft,
        onReopenSentDraft: _reopenCommunitySentDraft,
        onOpenMenu: () => _setDestination(_Destination.inventory),
        onCreateCustomer: _createCustomer,
      ),
      _Destination.kitchen => _KitchenWorkspace(
        key: const ValueKey('kitchen'),
        workspace: _workspace,
        canOperateKitchen: canKitchen,
        isSaving: _isSaving,
        onAdvance: _advanceKitchenTicket,
        onAcknowledgeCancellation: _acknowledgeKitchenTicketCancellation,
      ),
      _Destination.inventory => _CatalogWorkspace(
        key: const ValueKey('inventory'),
        workspace: _workspace,
        canManageCatalog: canManage,
        isSaving: _isSaving,
        applicationSupportDirectory: widget.applicationSupportDirectory,
        onCompleteSetup: _completeCommunitySetup,
        onCreateCategory: _createCategory,
        onCreateProduct: _createProduct,
        onUpdateProductPrice: _updateProductPrice,
        onSetProductAvailability: _setProductAvailability,
        onSetProductTaxTreatment: _setProductTaxTreatment,
        onArchiveProduct: _archiveProduct,
        onArchiveCategory: _archiveCategory,
        onReplaceProductImage: _replaceProductImage,
        onDeleteUnusedProduct: _deleteUnusedProduct,
        onCreateProductModifierOption: _createProductModifierOption,
        onArchiveProductModifierOption: _archiveProductModifierOption,
        onReviseCustomer: _reviseCustomer,
        onAnonymizeCustomer: _anonymizeCustomer,
      ),
      _Destination.reports => _ReportsWorkspace(
        key: const ValueKey('reports'),
        applicationSupportDirectory: widget.applicationSupportDirectory,
        activeStaffRole: role,
      ),
    };
  }

  Future<void> _retryLocalStorage() async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }
    setState(() => _isSaving = true);
    try {
      final workspace = await loadCommunityWorkspace(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      final security = await loadCommunityStaffSecurity(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      if (!mounted) return;
      setState(() {
        _workspace = workspace;
        _staffSecurity = security;
        if (!workspace.storageStatus.startsWith(
          'Local storage needs attention',
        )) {
          _destination = workspace.setupRequired
              ? _Destination.inventory
              : _Destination.overview;
        }
      });
    } finally {
      if (mounted) {
        setState(() => _isSaving = false);
      }
    }
  }

  Future<void> _resetStorageForFreshSetup() async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Start a fresh local workspace?'),
        content: const Text(
          'A secure device key is present, but the encrypted restaurant database file is missing. '
          'Starting fresh clears that orphaned key and creates an empty local workspace so you can set up again.\n\n'
          'Do this only if you do not have a portable recovery backup for this installation.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Start fresh'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) {
      return;
    }

    setState(() => _isSaving = true);
    try {
      final workspace = await resetCommunityStorageForFreshSetup(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      final security = await loadCommunityStaffSecurity(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      if (!mounted) return;
      setState(() {
        _workspace = workspace;
        _staffSecurity = security;
        _destination = workspace.setupRequired
            ? _Destination.inventory
            : _Destination.overview;
      });
      if (!workspace.storageStatus.startsWith(
        'Local storage needs attention',
      )) {
        ScaffoldMessenger.maybeOf(context)
          ?..clearSnackBars()
          ..showSnackBar(
            const SnackBar(
              content: Text(
                'Local storage is ready. Continue with restaurant setup.',
              ),
            ),
          );
      }
    } finally {
      if (mounted) {
        setState(() => _isSaving = false);
      }
    }
  }

  Future<void> _completeCommunitySetup({
    required String organizationName,
    required String branchName,
    required String currencyCode,
    required String timeZone,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await completeCommunitySetup(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        organizationName: organizationName,
        branchName: branchName,
        currencyCode: currencyCode,
        timeZone: timeZone,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
      await _refreshStaffSecurity(force: true);
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<void> _createCategory(String displayName) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await createCommunityCategory(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        displayName: displayName,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<void> _archiveCategory({
    required String categoryId,
    required int expectedRevision,
    required String reason,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await archiveCommunityCategory(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        categoryId: categoryId,
        expectedRevision: expectedRevision,
        reason: reason,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<void> _replaceProductImage({
    required String productId,
    Uint8List? restaurantImageBytes,
    String? builtInImageKey,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await replaceCommunityProductImage(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        productId: productId,
        restaurantImageBytes: restaurantImageBytes,
        builtInImageKey: builtInImageKey,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<void> _createCustomer({
    required String displayName,
    String? phoneNumber,
    String? emailAddress,
    required bool marketingConsent,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) return;
    setState(() => _isSaving = true);
    try {
      final workspace = await createCommunityCustomer(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        displayName: displayName,
        phoneNumber: phoneNumber,
        emailAddress: emailAddress,
        marketingConsent: marketingConsent,
      );
      if (mounted) setState(() => _workspace = workspace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _reviseCustomer({
    required String customerId,
    required String displayName,
    String? phoneNumber,
    String? emailAddress,
    required bool marketingConsent,
    required String reason,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) return;
    setState(() => _isSaving = true);
    try {
      final workspace = await reviseCommunityCustomer(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        customerId: customerId,
        displayName: displayName,
        phoneNumber: phoneNumber,
        emailAddress: emailAddress,
        marketingConsent: marketingConsent,
        reason: reason,
      );
      if (mounted) setState(() => _workspace = workspace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _anonymizeCustomer({
    required String customerId,
    required String reason,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) return;
    setState(() => _isSaving = true);
    try {
      final workspace = await anonymizeCommunityCustomer(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        customerId: customerId,
        reason: reason,
      );
      if (mounted) setState(() => _workspace = workspace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _createProduct({
    required String displayName,
    required String categoryId,
    required int unitPriceMinor,
    String? builtInImageKey,
    Uint8List? userImageBytes,
    _RemoteMenuImageSelection? catalogImage,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await createCommunityProduct(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        displayName: displayName,
        categoryId: categoryId,
        unitPriceMinor: unitPriceMinor,
        builtInImageKey: builtInImageKey,
        restaurantImageBytes: catalogImage == null ? userImageBytes : null,
        gotiginCatalogImage: catalogImage == null
            ? null
            : GotiginCatalogMenuImageSelection(
                catalogImageId: catalogImage.image.imageId,
                originalImageBytes: catalogImage.bytes,
                contentSha256: catalogImage.image.contentSha256,
                licenceLabel: catalogImage.image.licence,
                licenceUrl: catalogImage.image.licenceUrl.toString(),
                serviceOrigin: remoteMenuImageCatalogOrigin,
                serviceSchemaVersion: 1,
              ),
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<void> _archiveProduct({
    required String productId,
    required int expectedRevision,
    required String reason,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await archiveCommunityProduct(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        productId: productId,
        expectedRevision: expectedRevision,
        reason: reason,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<void> _createProductModifierOption({
    required String productId,
    required String displayName,
    required int priceDeltaMinor,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) return;
    setState(() => _isSaving = true);
    try {
      final workspace = await createCommunityProductModifierOption(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        productId: productId,
        displayName: displayName,
        priceDeltaMinor: priceDeltaMinor,
      );
      if (mounted) setState(() => _workspace = workspace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _archiveProductModifierOption({
    required String modifierOptionId,
    required int expectedRevision,
    required String reason,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) return;
    setState(() => _isSaving = true);
    try {
      final workspace = await archiveCommunityProductModifierOption(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        modifierOptionId: modifierOptionId,
        expectedRevision: expectedRevision,
        reason: reason,
      );
      if (mounted) setState(() => _workspace = workspace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _updateProductPrice({
    required String productId,
    required int expectedRevision,
    required int unitPriceMinor,
    required String reason,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await updateCommunityProductPrice(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        productId: productId,
        expectedRevision: expectedRevision,
        unitPriceMinor: unitPriceMinor,
        reason: reason,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<void> _setProductAvailability({
    required String productId,
    required int expectedRevision,
    required bool isAvailable,
    required String reason,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await setCommunityProductAvailability(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        productId: productId,
        expectedRevision: expectedRevision,
        isAvailable: isAvailable,
        reason: reason,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<void> _setProductTaxTreatment({
    required String productId,
    required int expectedRevision,
    required String taxTreatment,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await setCommunityProductTaxTreatment(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        productId: productId,
        taxTreatment: taxTreatment,
        expectedRevision: expectedRevision,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<void> _deleteUnusedProduct({
    required String productId,
    required int expectedRevision,
    required String reason,
  }) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return;
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final workspace = await deleteUnusedCommunityProduct(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        productId: productId,
        expectedRevision: expectedRevision,
        reason: reason,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _workspace = workspace;
      });
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<PosPricingPreview> _previewCommunitySalePricing({
    required List<PosCartLine> lines,
    int? discountFixedMinor,
    int? discountPercentageBasisPoints,
    int? discountPercentageCapMinor,
    String? discountReason,
  }) async {
    if (widget.applicationSupportDirectory.isEmpty) {
      return const PosPricingPreview(
        available: false,
        status:
            'Sale needs attention • encrypted local storage is not available.',
        subtotalMinor: 0,
        discountMinor: 0,
        taxMinor: 0,
        payableMinor: 0,
      );
    }

    try {
      final preview = await previewCommunitySalePricing(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        cartLines: lines
            .map(
              (line) => CommunityCartLine(
                productId: line.productId,
                quantity: line.quantity,
                modifierOptionIds: line.modifierOptionIds,
              ),
            )
            .toList(growable: false),
        discountFixedMinor: discountFixedMinor,
        discountPercentageBasisPoints: discountPercentageBasisPoints,
        discountPercentageCapMinor: discountPercentageCapMinor,
        discountReason: discountReason,
      );
      return PosPricingPreview(
        available: preview.available,
        status: preview.storageStatus,
        subtotalMinor: preview.subtotalMinor,
        discountMinor: preview.discountMinor,
        taxMinor: preview.taxMinor,
        payableMinor: preview.payableMinor,
        currencyCode: preview.currencyCode,
      );
    } catch (_) {
      return const PosPricingPreview(
        available: false,
        status:
            'Sale needs attention • the payable total could not be calculated. Your cart is still here.',
        subtotalMinor: 0,
        discountMinor: 0,
        taxMinor: 0,
        payableMinor: 0,
      );
    }
  }

  Future<PosCheckoutResult> _completeCommunitySale(
    PosCheckoutRequest request,
  ) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return const PosCheckoutResult(
        recorded: false,
        status:
            'Sale needs attention • encrypted local storage is not available. Your cart is still here.',
      );
    }

    setState(() {
      _isSaving = true;
    });

    try {
      final result = await completeCommunitySale(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        cartLines: request.lines
            .map(
              (line) => CommunityCartLine(
                productId: line.productId,
                quantity: line.quantity,
                modifierOptionIds: line.modifierOptionIds,
              ),
            )
            .toList(growable: false),
        fulfillment: request.fulfillment,
        paymentMethod: request.paymentMethod,
        paymentAllocations: request.paymentAllocations,
        customerId: request.customerId,
        draftOrderId: request.draftOrderId,
        expectedDraftRevision: request.expectedDraftRevision,
        discountFixedMinor: request.discountFixedMinor,
        discountPercentageBasisPoints: request.discountPercentageBasisPoints,
        discountPercentageCapMinor: request.discountPercentageCapMinor,
        discountReason: request.discountReason,
      );

      final checkoutResult = PosCheckoutResult(
        recorded: result.completed,
        status: result.storageStatus,
        invoiceNumber: result.invoiceNumber,
        totalMinor: result.totalMinor,
        currencyCode: result.currencyCode,
        paymentMethod: result.paymentMethod,
      );
      if (checkoutResult.recorded) {
        final workspace = await loadCommunityWorkspace(
          applicationSupportDirectory: widget.applicationSupportDirectory,
        );
        if (mounted) setState(() => _workspace = workspace);
      }
      return checkoutResult;
    } catch (_) {
      return const PosCheckoutResult(
        recorded: false,
        status:
            'Sale needs attention • the local sale could not be recorded. Your cart is still here.',
      );
    } finally {
      if (mounted) {
        setState(() {
          _isSaving = false;
        });
      }
    }
  }

  Future<PosDraftResult> _saveCommunityDraftOrder(
    PosDraftRequest request,
  ) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return const PosDraftResult(
        saved: false,
        status:
            'Order needs attention • encrypted local storage is not available. Your cart is still here.',
      );
    }
    setState(() => _isSaving = true);
    try {
      final result = await saveCommunityDraftOrder(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        draftOrderId: request.draftOrderId,
        expectedRevision: request.expectedRevision,
        cartLines: request.lines
            .map(
              (line) => CommunityCartLine(
                productId: line.productId,
                quantity: line.quantity,
                modifierOptionIds: line.modifierOptionIds,
              ),
            )
            .toList(growable: false),
        fulfillment: request.fulfillment,
        tableName: request.tableName,
        kitchenNote: request.kitchenNote,
      );
      final draftResult = PosDraftResult(
        saved: result.saved,
        status: result.storageStatus,
        draftOrderId: result.draftOrderId,
        revision: result.revision,
      );
      if (draftResult.saved) {
        final workspace = await loadCommunityWorkspace(
          applicationSupportDirectory: widget.applicationSupportDirectory,
        );
        if (mounted) setState(() => _workspace = workspace);
      }
      return draftResult;
    } catch (_) {
      return const PosDraftResult(
        saved: false,
        status:
            'Order needs attention • the open order could not be saved. Your cart is still here.',
      );
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<PosDraftActionResult> _sendCommunityDraftToKitchen(
    String draftOrderId,
    int revision,
  ) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return const PosDraftActionResult(
        succeeded: false,
        status:
            'Kitchen needs attention • encrypted local storage is unavailable.',
      );
    }
    setState(() => _isSaving = true);
    try {
      final workspace = await sendCommunityDraftToKitchen(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        draftOrderId: draftOrderId,
        expectedRevision: revision,
      );
      if (mounted) setState(() => _workspace = workspace);
      return PosDraftActionResult(
        succeeded: workspace.openDrafts.any(
          (draft) =>
              draft.draftOrderId == draftOrderId &&
              draft.revision == revision &&
              draft.draftState == 'sent_to_kitchen',
        ),
        status: workspace.storageStatus,
      );
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _cancelCommunityOpenDraft(
    String draftOrderId,
    int revision,
    String reason,
  ) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) return;
    setState(() => _isSaving = true);
    try {
      final workspace = await cancelCommunityOpenDraftOrder(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        draftOrderId: draftOrderId,
        expectedRevision: revision,
        reason: reason,
      );
      if (mounted) setState(() => _workspace = workspace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<PosDraftActionResult> _cancelCommunitySentDraft(
    String draftOrderId,
    int revision,
    String reason,
  ) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return const PosDraftActionResult(
        succeeded: false,
        status:
            'Kitchen cancellation needs attention • encrypted local storage is unavailable.',
      );
    }
    setState(() => _isSaving = true);
    try {
      final workspace = await cancelCommunitySentDraftOrder(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        draftOrderId: draftOrderId,
        expectedRevision: revision,
        reason: reason,
      );
      if (mounted) setState(() => _workspace = workspace);
      final succeeded =
          !workspace.setupRequired &&
          !workspace.storageStatus.toLowerCase().contains('attention') &&
          !workspace.openDrafts.any(
            (draft) =>
                draft.draftOrderId == draftOrderId &&
                draft.revision == revision &&
                draft.draftState == 'sent_to_kitchen',
          );
      return PosDraftActionResult(
        succeeded: succeeded,
        status: workspace.storageStatus,
      );
    } catch (_) {
      return const PosDraftActionResult(
        succeeded: false,
        status:
            'Kitchen cancellation needs attention • the stop-work notice was not saved.',
      );
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<PosDraftActionResult> _reopenCommunitySentDraft(
    String draftOrderId,
    int revision,
    String reason,
  ) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) {
      return const PosDraftActionResult(
        succeeded: false,
        status:
            'Order revision needs attention • encrypted local storage is unavailable.',
      );
    }
    setState(() => _isSaving = true);
    try {
      final workspace = await reopenCommunitySentDraftOrder(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        draftOrderId: draftOrderId,
        expectedRevision: revision,
        reason: reason,
      );
      if (mounted) setState(() => _workspace = workspace);
      final succeeded =
          !workspace.setupRequired &&
          !workspace.storageStatus.toLowerCase().contains('attention') &&
          workspace.openDrafts.any(
            (draft) =>
                draft.draftOrderId == draftOrderId &&
                draft.revision == revision + 1 &&
                draft.draftState == 'open',
          );
      return PosDraftActionResult(
        succeeded: succeeded,
        status: workspace.storageStatus,
      );
    } catch (_) {
      return const PosDraftActionResult(
        succeeded: false,
        status:
            'Order revision needs attention • the sent order was not reopened.',
      );
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _acknowledgeKitchenTicketCancellation(String ticketId) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) return;
    setState(() => _isSaving = true);
    try {
      final workspace = await acknowledgeCommunityKitchenTicketCancellation(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        ticketId: ticketId,
      );
      if (mounted) setState(() => _workspace = workspace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _advanceKitchenTicket(
    String ticketId,
    int revision,
    String state,
  ) async {
    if (_isSaving || widget.applicationSupportDirectory.isEmpty) return;
    setState(() => _isSaving = true);
    try {
      final workspace = await advanceCommunityKitchenTicket(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        ticketId: ticketId,
        expectedRevision: revision,
        nextState: state,
      );
      if (mounted) setState(() => _workspace = workspace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }
}

/// The local counter remains fail-closed until Rust has associated the device
/// with an active, short-lived staff session. This screen deliberately knows
/// only staff labels and roles; PIN verification and rate limiting stay in the
/// encrypted Rust storage layer.
class _StaffSecurityGate extends StatefulWidget {
  const _StaffSecurityGate({
    required this.security,
    required this.isSaving,
    required this.onConfigureOwnerPin,
    required this.onUnlock,
    required this.onRetry,
  });

  final CommunityStaffSecurity security;
  final bool isSaving;
  final Future<void> Function(String pin) onConfigureOwnerPin;
  final Future<void> Function(String staffId, String pin) onUnlock;
  final Future<void> Function() onRetry;

  @override
  State<_StaffSecurityGate> createState() => _StaffSecurityGateState();
}

class _StaffSecurityGateState extends State<_StaffSecurityGate> {
  final _formKey = GlobalKey<FormState>();
  final _pinController = TextEditingController();
  final _confirmPinController = TextEditingController();
  String? _selectedStaffId;

  @override
  void initState() {
    super.initState();
    _selectFirstEligibleStaff();
  }

  @override
  void didUpdateWidget(covariant _StaffSecurityGate oldWidget) {
    super.didUpdateWidget(oldWidget);
    _selectFirstEligibleStaff();
  }

  @override
  void dispose() {
    _pinController.dispose();
    _confirmPinController.dispose();
    super.dispose();
  }

  void _selectFirstEligibleStaff() {
    final eligible = widget.security.staff
        .where((staff) => staff.active && staff.pinConfigured)
        .toList(growable: false);
    if (eligible.any((staff) => staff.staffId == _selectedStaffId)) {
      return;
    }
    _selectedStaffId = eligible.isEmpty ? null : eligible.first.staffId;
  }

  @override
  Widget build(BuildContext context) {
    final security = widget.security;
    final setup = security.ownerPinSetupRequired;
    final unavailable = !security.available;
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 480),
              child: Card(
                child: Padding(
                  padding: const EdgeInsets.all(28),
                  child: unavailable
                      ? _UnavailableSecurityContent(
                          status: security.storageStatus,
                          busy: widget.isSaving,
                          onRetry: widget.onRetry,
                        )
                      : Form(
                          key: _formKey,
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Icon(
                                setup
                                    ? Icons.admin_panel_settings_outlined
                                    : Icons.lock_outline,
                                size: 36,
                                color: Theme.of(context).colorScheme.primary,
                              ),
                              const SizedBox(height: 18),
                              Text(
                                setup
                                    ? 'Secure your restaurant'
                                    : 'Unlock Restaurant Operating System',
                                style: Theme.of(context).textTheme.headlineSmall
                                    ?.copyWith(fontWeight: FontWeight.w800),
                              ),
                              const SizedBox(height: 8),
                              Text(
                                setup
                                    ? 'Create the owner PIN for this device. It protects operational changes and is stored only as an Argon2id verifier.'
                                    : 'Choose your staff account and enter its PIN. The session expires after 15 minutes or when you lock it.',
                                style: Theme.of(context).textTheme.bodyMedium
                                    ?.copyWith(
                                      color: Theme.of(
                                        context,
                                      ).colorScheme.onSurfaceVariant,
                                    ),
                              ),
                              const SizedBox(height: 22),
                              _SecurityStatus(status: security.storageStatus),
                              const SizedBox(height: 20),
                              if (setup) ...[
                                TextFormField(
                                  key: const Key('owner-pin'),
                                  controller: _pinController,
                                  enabled: !widget.isSaving,
                                  autofocus: true,
                                  obscureText: true,
                                  keyboardType: TextInputType.number,
                                  maxLength: 12,
                                  decoration: const InputDecoration(
                                    labelText: 'Owner PIN',
                                    hintText: '6 to 12 digits',
                                    prefixIcon: Icon(Icons.password_outlined),
                                  ),
                                  validator: _pinValidator,
                                ),
                                const SizedBox(height: 12),
                                TextFormField(
                                  key: const Key('owner-pin-confirm'),
                                  controller: _confirmPinController,
                                  enabled: !widget.isSaving,
                                  obscureText: true,
                                  keyboardType: TextInputType.number,
                                  maxLength: 12,
                                  decoration: const InputDecoration(
                                    labelText: 'Confirm owner PIN',
                                    prefixIcon: Icon(Icons.password_outlined),
                                  ),
                                  validator: (value) =>
                                      value == _pinController.text
                                      ? null
                                      : 'PINs do not match',
                                ),
                              ] else ...[
                                DropdownButtonFormField<String>(
                                  key: const Key('staff-selector'),
                                  initialValue: _selectedStaffId,
                                  decoration: const InputDecoration(
                                    labelText: 'Staff member',
                                    prefixIcon: Icon(Icons.badge_outlined),
                                  ),
                                  onChanged: widget.isSaving
                                      ? null
                                      : (value) => setState(
                                          () => _selectedStaffId = value,
                                        ),
                                  items: security.staff
                                      .where(
                                        (staff) =>
                                            staff.active && staff.pinConfigured,
                                      )
                                      .map(
                                        (staff) => DropdownMenuItem(
                                          value: staff.staffId,
                                          child: Text(
                                            '${staff.displayName} • ${_roleLabel(staff.role)}',
                                          ),
                                        ),
                                      )
                                      .toList(growable: false),
                                ),
                                const SizedBox(height: 12),
                                TextFormField(
                                  key: const Key('staff-pin'),
                                  controller: _pinController,
                                  enabled:
                                      !widget.isSaving &&
                                      _selectedStaffId != null,
                                  autofocus: true,
                                  obscureText: true,
                                  keyboardType: TextInputType.number,
                                  maxLength: 12,
                                  decoration: const InputDecoration(
                                    labelText: 'PIN',
                                    hintText: '6 to 12 digits',
                                    prefixIcon: Icon(Icons.password_outlined),
                                  ),
                                  validator: _pinValidator,
                                ),
                              ],
                              const SizedBox(height: 24),
                              Align(
                                alignment: Alignment.centerRight,
                                child: FilledButton.icon(
                                  key: Key(
                                    setup
                                        ? 'configure-owner-pin'
                                        : 'unlock-staff-session',
                                  ),
                                  onPressed: widget.isSaving
                                      ? null
                                      : setup
                                      ? _configureOwnerPin
                                      : _unlock,
                                  icon: widget.isSaving
                                      ? const SizedBox(
                                          height: 18,
                                          width: 18,
                                          child: CircularProgressIndicator(
                                            strokeWidth: 2,
                                          ),
                                        )
                                      : Icon(
                                          setup
                                              ? Icons.security_outlined
                                              : Icons.lock_open_outlined,
                                        ),
                                  label: Text(
                                    widget.isSaving
                                        ? 'Working securely…'
                                        : setup
                                        ? 'Secure this device'
                                        : 'Unlock',
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  String? _pinValidator(String? value) {
    final pin = value ?? '';
    if (!RegExp(r'^\d{6,12}$').hasMatch(pin)) {
      return 'Use 6 to 12 digits';
    }
    return null;
  }

  Future<void> _configureOwnerPin() async {
    if (!(_formKey.currentState?.validate() ?? false)) {
      return;
    }
    await widget.onConfigureOwnerPin(_pinController.text);
  }

  Future<void> _unlock() async {
    if (!(_formKey.currentState?.validate() ?? false) ||
        _selectedStaffId == null) {
      return;
    }
    await widget.onUnlock(_selectedStaffId!, _pinController.text);
  }
}

class _UnavailableSecurityContent extends StatelessWidget {
  const _UnavailableSecurityContent({
    required this.status,
    required this.busy,
    required this.onRetry,
  });

  final String status;
  final bool busy;
  final Future<void> Function() onRetry;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(
          Icons.shield_outlined,
          size: 36,
          color: Theme.of(context).colorScheme.error,
        ),
        const SizedBox(height: 18),
        Text(
          'Staff security needs attention',
          style: Theme.of(
            context,
          ).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800),
        ),
        const SizedBox(height: 8),
        Text(status),
        const SizedBox(height: 22),
        FilledButton.icon(
          onPressed: busy ? null : onRetry,
          icon: const Icon(Icons.refresh),
          label: const Text('Retry secure storage'),
        ),
      ],
    );
  }
}

class _SecurityStatus extends StatelessWidget {
  const _SecurityStatus({required this.status});

  final String status;

  @override
  Widget build(BuildContext context) => Container(
    width: double.infinity,
    padding: const EdgeInsets.all(12),
    decoration: BoxDecoration(
      color: Theme.of(context).colorScheme.secondaryContainer,
      borderRadius: const BorderRadius.all(Radius.circular(12)),
    ),
    child: Text(
      status,
      style: Theme.of(context).textTheme.bodySmall?.copyWith(
        color: Theme.of(context).colorScheme.onSecondaryContainer,
      ),
    ),
  );
}

String _roleLabel(String role) => switch (role) {
  'owner' => 'Owner',
  'manager' => 'Manager',
  'cashier' => 'Cashier',
  'kitchen' => 'Kitchen',
  _ => 'Staff',
};

class _Sidebar extends StatelessWidget {
  const _Sidebar({
    required this.destination,
    required this.onChanged,
    required this.showLocalOnlyBadge,
  });

  final _Destination destination;
  final ValueChanged<_Destination> onChanged;
  final bool showLocalOnlyBadge;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 248,
      decoration: BoxDecoration(
        color: Colors.white,
        border: Border(
          right: BorderSide(
            color: Theme.of(context).colorScheme.outlineVariant,
          ),
        ),
      ),
      child: Column(
        children: [
          const Padding(
            padding: EdgeInsets.fromLTRB(24, 30, 24, 26),
            child: _Brand(),
          ),
          Expanded(
            child: NavigationRail(
              extended: true,
              minExtendedWidth: 248,
              selectedIndex: _Destination.values.indexOf(destination),
              onDestinationSelected: (index) {
                onChanged(_Destination.values[index]);
              },
              backgroundColor: Colors.transparent,
              labelType: NavigationRailLabelType.none,
              destinations: const [
                NavigationRailDestination(
                  icon: Icon(Icons.space_dashboard_outlined),
                  selectedIcon: Icon(Icons.space_dashboard),
                  label: Text('Overview'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.point_of_sale_outlined),
                  selectedIcon: Icon(Icons.point_of_sale),
                  label: Text('Point of Sale'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.soup_kitchen_outlined),
                  selectedIcon: Icon(Icons.soup_kitchen),
                  label: Text('Kitchen Display'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.menu_book_outlined),
                  selectedIcon: Icon(Icons.menu_book),
                  label: Text('Menu & categories'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.insights_outlined),
                  selectedIcon: Icon(Icons.insights),
                  label: Text('Reports'),
                ),
              ],
            ),
          ),
          if (showLocalOnlyBadge)
            const Padding(
              padding: EdgeInsets.all(20),
              child: _LocalOnlyBadge(),
            ),
        ],
      ),
    );
  }
}

class _Overview extends StatelessWidget {
  const _Overview({
    required this.coreStatus,
    required this.setupRequired,
    this.branchName,
    required this.openOrderCount,
    required this.kitchenQueueCount,
    required this.onOpenPos,
    required this.onStartSetup,
    super.key,
  });

  final String coreStatus;
  final bool setupRequired;
  final String? branchName;
  final int openOrderCount;
  final int kitchenQueueCount;
  final VoidCallback onOpenPos;
  final VoidCallback onStartSetup;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).extension<RestaurantColors>()!;
    final isCompact = MediaQuery.sizeOf(context).width < 700;

    return CustomScrollView(
      key: const PageStorageKey('overview-scroll'),
      slivers: [
        SliverPadding(
          padding: EdgeInsets.fromLTRB(
            isCompact ? 20 : 44,
            28,
            isCompact ? 20 : 44,
            40,
          ),
          sliver: SliverList(
            delegate: SliverChildListDelegate([
              _TopBar(isCompact: isCompact),
              const SizedBox(height: 34),
              Text(
                setupRequired
                    ? 'Let’s set up your restaurant.'
                    : '${branchName ?? 'Your restaurant'} is ready for service.',
                style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                  fontWeight: FontWeight.w700,
                  letterSpacing: -0.8,
                ),
              ),
              const SizedBox(height: 8),
              Text(
                setupRequired
                    ? 'Your encrypted local workspace is ready when you are.'
                    : 'Start a local order, watch kitchen progress, and review verified reports.',
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: 26),
              _ServiceReadyCard(
                coreStatus: coreStatus,
                setupRequired: setupRequired,
                onOpenPos: onOpenPos,
                onStartSetup: onStartSetup,
                mint: colors.mint,
              ),
              const SizedBox(height: 26),
              _MetricsGrid(
                openOrderCount: openOrderCount,
                kitchenQueueCount: kitchenQueueCount,
              ),
              const SizedBox(height: 26),
              _ServiceSnapshot(isCompact: isCompact),
            ]),
          ),
        ),
      ],
    );
  }
}

class _TopBar extends StatelessWidget {
  const _TopBar({required this.isCompact});

  final bool isCompact;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        if (isCompact) const Expanded(child: _Brand()),
        if (!isCompact)
          Expanded(
            child: Text(
              'Service command center',
              style: Theme.of(
                context,
              ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700),
            ),
          ),
        IconButton(
          tooltip: 'Notifications',
          onPressed: () {},
          icon: const Badge(
            smallSize: 8,
            child: Icon(Icons.notifications_none),
          ),
        ),
        const SizedBox(width: 8),
        const CircleAvatar(
          radius: 19,
          backgroundColor: Color(0xFFDDECE2),
          child: Text(
            'P',
            style: TextStyle(
              color: Color(0xFF126B4A),
              fontWeight: FontWeight.w800,
            ),
          ),
        ),
      ],
    );
  }
}

class _Brand extends StatelessWidget {
  const _Brand();

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Container(
          height: 38,
          width: 38,
          decoration: const BoxDecoration(
            color: Color(0xFF126B4A),
            borderRadius: BorderRadius.all(Radius.circular(12)),
          ),
          child: const Icon(
            Icons.restaurant_menu,
            color: Colors.white,
            size: 21,
          ),
        ),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'GOTIGIN',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.labelLarge?.copyWith(
                  color: const Color(0xFF126B4A),
                  fontWeight: FontWeight.w900,
                  letterSpacing: 1.5,
                ),
              ),
              Text(
                'Restaurant Operating System',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(
                  context,
                ).textTheme.labelMedium?.copyWith(fontWeight: FontWeight.w600),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _LocalOnlyBadge extends StatelessWidget {
  const _LocalOnlyBadge();

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: const BoxDecoration(
        color: Color(0xFFE6F4EC),
        borderRadius: BorderRadius.all(Radius.circular(14)),
      ),
      child: const Padding(
        padding: EdgeInsets.all(12),
        child: Row(
          children: [
            Icon(Icons.shield_outlined, size: 18, color: Color(0xFF126B4A)),
            SizedBox(width: 8),
            Expanded(
              child: Text(
                'Community Edition\nLocal-first and always yours',
                style: TextStyle(
                  color: Color(0xFF126B4A),
                  fontSize: 12,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ServiceReadyCard extends StatelessWidget {
  const _ServiceReadyCard({
    required this.coreStatus,
    required this.setupRequired,
    required this.onOpenPos,
    required this.onStartSetup,
    required this.mint,
  });

  final String coreStatus;
  final bool setupRequired;
  final VoidCallback onOpenPos;
  final VoidCallback onStartSetup;
  final Color mint;

  @override
  Widget build(BuildContext context) {
    return Card(
      color: mint,
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Wrap(
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          runSpacing: 20,
          spacing: 20,
          children: [
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 540),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const _StatusPill(
                    icon: Icons.shield_outlined,
                    label: 'LOCAL-FIRST',
                  ),
                  const SizedBox(height: 16),
                  Text(
                    setupRequired
                        ? 'Set up once. Keep working offline.'
                        : 'Every order is saved locally first.',
                    style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                      letterSpacing: -0.5,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    setupRequired
                        ? 'Create your restaurant and its first menu categories without creating a cloud account.'
                        : 'Take orders confidently, even when the network is unavailable.',
                    style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  ),
                  const SizedBox(height: 14),
                  Semantics(
                    label: 'Rust operational core status',
                    child: Text(
                      coreStatus,
                      style: Theme.of(context).textTheme.labelMedium?.copyWith(
                        color: const Color(0xFF126B4A),
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                ],
              ),
            ),
            FilledButton.icon(
              onPressed: setupRequired ? onStartSetup : onOpenPos,
              icon: Icon(setupRequired ? Icons.storefront_outlined : Icons.add),
              label: Text(setupRequired ? 'Set up restaurant' : 'New order'),
              style: FilledButton.styleFrom(minimumSize: const Size(136, 52)),
            ),
          ],
        ),
      ),
    );
  }
}

class _StatusPill extends StatelessWidget {
  const _StatusPill({required this.icon, required this.label});

  final IconData icon;
  final String label;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: const BoxDecoration(
        color: Colors.white,
        borderRadius: BorderRadius.all(Radius.circular(999)),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, color: const Color(0xFF126B4A), size: 16),
            const SizedBox(width: 6),
            Text(
              label,
              style: const TextStyle(
                color: Color(0xFF126B4A),
                fontSize: 11,
                fontWeight: FontWeight.w900,
                letterSpacing: 0.7,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _MetricsGrid extends StatelessWidget {
  const _MetricsGrid({
    required this.openOrderCount,
    required this.kitchenQueueCount,
  });

  final int openOrderCount;
  final int kitchenQueueCount;

  @override
  Widget build(BuildContext context) {
    final cards = [
      const _Metric(
        label: 'Sales reports',
        value: 'Local',
        detail: 'Verified totals live in Reports',
        icon: Icons.payments_outlined,
      ),
      _Metric(
        label: 'Open orders',
        value: '$openOrderCount',
        detail: openOrderCount == 0
            ? 'No tables or holds waiting'
            : '$openOrderCount retained order${openOrderCount == 1 ? '' : 's'}',
        icon: Icons.receipt_long_outlined,
      ),
      _Metric(
        label: 'Kitchen queue',
        value: '$kitchenQueueCount',
        detail: kitchenQueueCount == 0
            ? 'All clear'
            : '$kitchenQueueCount active ticket${kitchenQueueCount == 1 ? '' : 's'}',
        icon: Icons.soup_kitchen_outlined,
      ),
      const _Metric(
        label: 'Sync state',
        value: 'Local',
        detail: 'Cloud is optional',
        icon: Icons.devices_outlined,
      ),
    ];

    // The sidebar reduces the usable content width on a desktop window. Use
    // the actual grid width—not the full window width—to avoid four cramped
    // metric cards in the mid-width desktop range.
    return LayoutBuilder(
      builder: (context, constraints) {
        final textScale = MediaQuery.textScalerOf(context).scale(1);
        // Large accessibility text needs wider cards and extra vertical room.
        // Otherwise each individual card can overflow even though the grid
        // itself has enough width for the usual desktop layout.
        final crossAxisCount = textScale >= 1.4
            ? (constraints.maxWidth >= 760 ? 2 : 1)
            : switch (constraints.maxWidth) {
                >= 960 => 4,
                >= 720 => 3,
                _ => 2,
              };
        final extraHeight = ((textScale - 1).clamp(0.0, 1.0) * 76).toDouble();

        return GridView.builder(
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          itemCount: cards.length,
          gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
            crossAxisCount: crossAxisCount,
            mainAxisSpacing: 14,
            crossAxisSpacing: 14,
            // The base height keeps ordinary cards compact; the scale-aware
            // addition keeps the icon, value, label, and status visible.
            mainAxisExtent: 154 + extraHeight,
          ),
          itemBuilder: (context, index) => cards[index],
        );
      },
    );
  }
}

class _Metric extends StatelessWidget {
  const _Metric({
    required this.label,
    required this.value,
    required this.detail,
    required this.icon,
  });

  final String label;
  final String value;
  final String detail;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(18),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(icon, color: Theme.of(context).colorScheme.primary),
            const Spacer(),
            Text(
              value,
              style: Theme.of(
                context,
              ).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 2),
            Text(
              label,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(
                context,
              ).textTheme.labelLarge?.copyWith(fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 3),
            Text(
              detail,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ServiceSnapshot extends StatelessWidget {
  const _ServiceSnapshot({required this.isCompact});

  final bool isCompact;

  @override
  Widget build(BuildContext context) {
    final activity = Card(
      child: Padding(
        padding: const EdgeInsets.all(22),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Today’s activity',
              style: Theme.of(
                context,
              ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 8),
            Text(
              'Your audit timeline will appear here after the first restaurant action.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 24),
            const _TimelineRow(
              icon: Icons.shield_outlined,
              title: 'Secure by design',
              detail:
                  'Final invoices and payments are corrected through traceable events.',
            ),
            const SizedBox(height: 20),
            const _TimelineRow(
              icon: Icons.cloud_off_outlined,
              title: 'Offline-first',
              detail:
                  'Cloud sync adds visibility; it never decides whether you can bill.',
            ),
          ],
        ),
      ),
    );

    final quickActions = Card(
      child: Padding(
        padding: const EdgeInsets.all(22),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Set up your service',
              style: Theme.of(
                context,
              ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 18),
            const _SetupStep(
              number: '01',
              title: 'Add menu categories',
              complete: true,
            ),
            const _SetupStep(
              number: '02',
              title: 'Add your first menu item',
              complete: false,
            ),
            const _SetupStep(
              number: '03',
              title: 'Create tables and staff',
              complete: false,
            ),
          ],
        ),
      ),
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        // As with the metric grid, a visible desktop sidebar can leave less
        // content width than the overall window suggests.
        if (isCompact || constraints.maxWidth < 720) {
          return Column(
            children: [activity, const SizedBox(height: 16), quickActions],
          );
        }

        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(flex: 6, child: activity),
            const SizedBox(width: 16),
            Expanded(flex: 4, child: quickActions),
          ],
        );
      },
    );
  }
}

class _TimelineRow extends StatelessWidget {
  const _TimelineRow({
    required this.icon,
    required this.title,
    required this.detail,
  });

  final IconData icon;
  final String title;
  final String detail;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          height: 36,
          width: 36,
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.primaryContainer,
            borderRadius: const BorderRadius.all(Radius.circular(12)),
          ),
          child: Icon(
            icon,
            color: Theme.of(context).colorScheme.primary,
            size: 19,
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                title,
                style: Theme.of(
                  context,
                ).textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w800),
              ),
              const SizedBox(height: 3),
              Text(
                detail,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _SetupStep extends StatelessWidget {
  const _SetupStep({
    required this.number,
    required this.title,
    required this.complete,
  });

  final String number;
  final String title;
  final bool complete;

  @override
  Widget build(BuildContext context) {
    final color = complete
        ? Theme.of(context).colorScheme.primary
        : Theme.of(context).colorScheme.outline;

    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Row(
        children: [
          Container(
            height: 30,
            width: 30,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: complete
                  ? Theme.of(context).colorScheme.primaryContainer
                  : Colors.transparent,
              border: Border.all(color: color),
              borderRadius: const BorderRadius.all(Radius.circular(10)),
            ),
            child: complete
                ? Icon(
                    Icons.check,
                    color: Theme.of(context).colorScheme.primary,
                    size: 17,
                  )
                : Text(
                    number,
                    style: Theme.of(context).textTheme.labelSmall?.copyWith(
                      color: color,
                      fontWeight: FontWeight.w800,
                    ),
                  ),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              title,
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: complete
                    ? Theme.of(context).colorScheme.onSurface
                    : Theme.of(context).colorScheme.onSurfaceVariant,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

typedef _CommunitySetupSubmit =
    Future<void> Function({
      required String organizationName,
      required String branchName,
      required String currencyCode,
      required String timeZone,
    });

typedef _CreateCategory = Future<void> Function(String displayName);

typedef _ArchiveCategory =
    Future<void> Function({
      required String categoryId,
      required int expectedRevision,
      required String reason,
    });

typedef _ReplaceProductImage =
    Future<void> Function({
      required String productId,
      Uint8List? restaurantImageBytes,
      String? builtInImageKey,
    });

typedef _CreateProduct =
    Future<void> Function({
      required String displayName,
      required String categoryId,
      required int unitPriceMinor,
      String? builtInImageKey,
      Uint8List? userImageBytes,
      _RemoteMenuImageSelection? catalogImage,
    });

typedef _ArchiveProduct =
    Future<void> Function({
      required String productId,
      required int expectedRevision,
      required String reason,
    });

typedef _UpdateProductPrice =
    Future<void> Function({
      required String productId,
      required int expectedRevision,
      required int unitPriceMinor,
      required String reason,
    });

typedef _SetProductAvailability =
    Future<void> Function({
      required String productId,
      required int expectedRevision,
      required bool isAvailable,
      required String reason,
    });

typedef _SetProductTaxTreatment =
    Future<void> Function({
      required String productId,
      required int expectedRevision,
      required String taxTreatment,
    });

typedef _DeleteUnusedProduct =
    Future<void> Function({
      required String productId,
      required int expectedRevision,
      required String reason,
    });

typedef _CreateProductModifierOption =
    Future<void> Function({
      required String productId,
      required String displayName,
      required int priceDeltaMinor,
    });

typedef _ArchiveProductModifierOption =
    Future<void> Function({
      required String modifierOptionId,
      required int expectedRevision,
      required String reason,
    });

typedef _ReviseCustomer =
    Future<void> Function({
      required String customerId,
      required String displayName,
      String? phoneNumber,
      String? emailAddress,
      required bool marketingConsent,
      required String reason,
    });

typedef _AnonymizeCustomer =
    Future<void> Function({required String customerId, required String reason});

class _CatalogWorkspace extends StatelessWidget {
  const _CatalogWorkspace({
    required this.workspace,
    required this.canManageCatalog,
    required this.isSaving,
    required this.applicationSupportDirectory,
    required this.onCompleteSetup,
    required this.onCreateCategory,
    required this.onArchiveCategory,
    required this.onCreateProduct,
    required this.onUpdateProductPrice,
    required this.onSetProductAvailability,
    required this.onSetProductTaxTreatment,
    required this.onArchiveProduct,
    required this.onReplaceProductImage,
    required this.onDeleteUnusedProduct,
    required this.onCreateProductModifierOption,
    required this.onArchiveProductModifierOption,
    required this.onReviseCustomer,
    required this.onAnonymizeCustomer,
    super.key,
  });

  final CommunityWorkspace workspace;
  final bool canManageCatalog;
  final bool isSaving;
  final String applicationSupportDirectory;
  final _CommunitySetupSubmit onCompleteSetup;
  final _CreateCategory onCreateCategory;
  final _ArchiveCategory onArchiveCategory;
  final _CreateProduct onCreateProduct;
  final _UpdateProductPrice onUpdateProductPrice;
  final _SetProductAvailability onSetProductAvailability;
  final _SetProductTaxTreatment onSetProductTaxTreatment;
  final _ArchiveProduct onArchiveProduct;
  final _ReplaceProductImage onReplaceProductImage;
  final _DeleteUnusedProduct onDeleteUnusedProduct;
  final _CreateProductModifierOption onCreateProductModifierOption;
  final _ArchiveProductModifierOption onArchiveProductModifierOption;
  final _ReviseCustomer onReviseCustomer;
  final _AnonymizeCustomer onAnonymizeCustomer;

  @override
  Widget build(BuildContext context) {
    return AnimatedSwitcher(
      duration: const Duration(milliseconds: 180),
      child: workspace.setupRequired
          ? _CommunitySetupForm(
              key: const ValueKey('community-setup'),
              status: workspace.storageStatus,
              isSaving: isSaving,
              onSubmit: onCompleteSetup,
            )
          : !canManageCatalog
          ? const _FeatureCanvas(
              icon: Icons.lock_outline,
              title: 'Menu management is restricted',
              description:
                  'Unlock as an owner or manager to change the menu, prices, images, or stock.',
            )
          : _CategoryManager(
              key: const ValueKey('category-manager'),
              workspace: workspace,
              isSaving: isSaving,
              applicationSupportDirectory: applicationSupportDirectory,
              onCreateCategory: onCreateCategory,
              onArchiveCategory: onArchiveCategory,
              onCreateProduct: onCreateProduct,
              onUpdateProductPrice: onUpdateProductPrice,
              onSetProductAvailability: onSetProductAvailability,
              onSetProductTaxTreatment: onSetProductTaxTreatment,
              onArchiveProduct: onArchiveProduct,
              onReplaceProductImage: onReplaceProductImage,
              onDeleteUnusedProduct: onDeleteUnusedProduct,
              onCreateProductModifierOption: onCreateProductModifierOption,
              onArchiveProductModifierOption: onArchiveProductModifierOption,
              onReviseCustomer: onReviseCustomer,
              onAnonymizeCustomer: onAnonymizeCustomer,
            ),
    );
  }
}

class _StorageAttentionWorkspace extends StatelessWidget {
  const _StorageAttentionWorkspace({
    required this.status,
    required this.isSaving,
    required this.canStartFresh,
    required this.onRetry,
    required this.onStartFresh,
    super.key,
  });

  final String status;
  final bool isSaving;
  final bool canStartFresh;
  final VoidCallback onRetry;
  final VoidCallback onStartFresh;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final guidance = canStartFresh
        ? 'A secure device key is present, but the encrypted restaurant database file is missing. '
              'You can retry if the file was restored, or start a fresh local workspace to continue setup.'
        : 'Secure local storage could not be opened. No restaurant data has been created or changed. '
              'Retry after fixing device storage, or ask an owner to restore a portable backup.';

    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Padding(
          padding: const EdgeInsets.all(28),
          child: Card(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(28),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    Icons.shield_outlined,
                    color: theme.colorScheme.error,
                    size: 32,
                  ),
                  const SizedBox(height: 16),
                  Text(
                    'Local storage needs attention',
                    style: theme.textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  const SizedBox(height: 10),
                  Text(
                    status,
                    style: theme.textTheme.bodyLarge?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                  const SizedBox(height: 16),
                  Text(guidance, style: theme.textTheme.bodyMedium),
                  const SizedBox(height: 24),
                  Wrap(
                    spacing: 12,
                    runSpacing: 12,
                    children: [
                      FilledButton.icon(
                        onPressed: isSaving ? null : onRetry,
                        icon: isSaving
                            ? const SizedBox(
                                width: 16,
                                height: 16,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.refresh),
                        label: Text(isSaving ? 'Checking…' : 'Retry storage'),
                      ),
                      if (canStartFresh)
                        OutlinedButton.icon(
                          onPressed: isSaving ? null : onStartFresh,
                          icon: const Icon(Icons.restart_alt),
                          label: const Text('Start fresh setup'),
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

class _CommunitySetupForm extends StatefulWidget {
  const _CommunitySetupForm({
    required this.status,
    required this.isSaving,
    required this.onSubmit,
    super.key,
  });

  final String status;
  final bool isSaving;
  final _CommunitySetupSubmit onSubmit;

  @override
  State<_CommunitySetupForm> createState() => _CommunitySetupFormState();
}

class _CommunitySetupFormState extends State<_CommunitySetupForm> {
  final _formKey = GlobalKey<FormState>();
  final _organizationController = TextEditingController();
  final _branchController = TextEditingController();
  final _timeZoneController = TextEditingController(text: 'Asia/Kolkata');
  var _currencyCode = 'INR';

  @override
  void dispose() {
    _organizationController.dispose();
    _branchController.dispose();
    _timeZoneController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final isCompact = MediaQuery.sizeOf(context).width < 760;

    return CustomScrollView(
      key: const PageStorageKey('community-setup-scroll'),
      slivers: [
        SliverPadding(
          padding: EdgeInsets.fromLTRB(
            isCompact ? 20 : 44,
            28,
            isCompact ? 20 : 44,
            40,
          ),
          sliver: SliverToBoxAdapter(
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 720),
                child: Card(
                  child: Padding(
                    padding: const EdgeInsets.all(28),
                    child: Form(
                      key: _formKey,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Icon(
                            Icons.storefront_outlined,
                            color: Theme.of(context).colorScheme.primary,
                            size: 34,
                          ),
                          const SizedBox(height: 18),
                          Text(
                            'Create your local restaurant workspace',
                            style: Theme.of(context).textTheme.headlineSmall
                                ?.copyWith(fontWeight: FontWeight.w800),
                          ),
                          const SizedBox(height: 8),
                          Text(
                            'This creates one Community Edition restaurant on this device. Your data is encrypted locally and stays usable without a cloud account.',
                            style: Theme.of(context).textTheme.bodyLarge
                                ?.copyWith(
                                  color: Theme.of(
                                    context,
                                  ).colorScheme.onSurfaceVariant,
                                ),
                          ),
                          const SizedBox(height: 22),
                          _WorkspaceStatus(status: widget.status),
                          const SizedBox(height: 24),
                          TextFormField(
                            controller: _organizationController,
                            enabled: !widget.isSaving,
                            autofocus: true,
                            decoration: const InputDecoration(
                              labelText: 'Restaurant or company name',
                              hintText: 'e.g. Saffron Table',
                              prefixIcon: Icon(Icons.business_outlined),
                            ),
                            textCapitalization: TextCapitalization.words,
                            validator: _requiredField,
                          ),
                          const SizedBox(height: 16),
                          TextFormField(
                            controller: _branchController,
                            enabled: !widget.isSaving,
                            decoration: const InputDecoration(
                              labelText: 'First branch name',
                              hintText: 'e.g. Indiranagar',
                              prefixIcon: Icon(Icons.location_on_outlined),
                            ),
                            textCapitalization: TextCapitalization.words,
                            validator: _requiredField,
                          ),
                          const SizedBox(height: 16),
                          if (isCompact)
                            Column(
                              children: [
                                _CurrencyField(
                                  currencyCode: _currencyCode,
                                  enabled: !widget.isSaving,
                                  onChanged: (value) {
                                    if (value != null) {
                                      setState(() {
                                        _currencyCode = value;
                                      });
                                    }
                                  },
                                ),
                                const SizedBox(height: 16),
                                _TimeZoneField(
                                  controller: _timeZoneController,
                                  enabled: !widget.isSaving,
                                ),
                              ],
                            )
                          else
                            Row(
                              children: [
                                Expanded(
                                  child: _CurrencyField(
                                    currencyCode: _currencyCode,
                                    enabled: !widget.isSaving,
                                    onChanged: (value) {
                                      if (value != null) {
                                        setState(() {
                                          _currencyCode = value;
                                        });
                                      }
                                    },
                                  ),
                                ),
                                const SizedBox(width: 16),
                                Expanded(
                                  child: _TimeZoneField(
                                    controller: _timeZoneController,
                                    enabled: !widget.isSaving,
                                  ),
                                ),
                              ],
                            ),
                          const SizedBox(height: 28),
                          Align(
                            alignment: Alignment.centerRight,
                            child: FilledButton.icon(
                              onPressed: widget.isSaving ? null : _submit,
                              icon: widget.isSaving
                                  ? const SizedBox(
                                      height: 18,
                                      width: 18,
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      ),
                                    )
                                  : const Icon(Icons.lock_outline),
                              label: Text(
                                widget.isSaving
                                    ? 'Saving securely…'
                                    : 'Create local workspace',
                              ),
                              style: FilledButton.styleFrom(
                                minimumSize: const Size(218, 52),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Future<void> _submit() async {
    if (!(_formKey.currentState?.validate() ?? false)) {
      return;
    }

    await widget.onSubmit(
      organizationName: _organizationController.text,
      branchName: _branchController.text,
      currencyCode: _currencyCode,
      timeZone: _timeZoneController.text,
    );
  }

  String? _requiredField(String? value) {
    if (value == null || value.trim().isEmpty) {
      return 'This field is required';
    }
    return null;
  }
}

class _CurrencyField extends StatelessWidget {
  const _CurrencyField({
    required this.currencyCode,
    required this.enabled,
    required this.onChanged,
  });

  final String currencyCode;
  final bool enabled;
  final ValueChanged<String?> onChanged;

  @override
  Widget build(BuildContext context) {
    return DropdownButtonFormField<String>(
      initialValue: currencyCode,
      decoration: const InputDecoration(
        labelText: 'Operating currency',
        prefixIcon: Icon(Icons.currency_rupee),
      ),
      onChanged: enabled ? onChanged : null,
      items: const [DropdownMenuItem(value: 'INR', child: Text('INR'))],
    );
  }
}

class _TimeZoneField extends StatelessWidget {
  const _TimeZoneField({required this.controller, required this.enabled});

  final TextEditingController controller;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      controller: controller,
      enabled: enabled,
      decoration: const InputDecoration(
        labelText: 'Time zone',
        prefixIcon: Icon(Icons.schedule_outlined),
      ),
      validator: (value) {
        if (value == null || value.trim().isEmpty) {
          return 'This field is required';
        }
        return null;
      },
    );
  }
}

class _CategoryManager extends StatefulWidget {
  const _CategoryManager({
    required this.workspace,
    required this.isSaving,
    required this.applicationSupportDirectory,
    required this.onCreateCategory,
    required this.onArchiveCategory,
    required this.onCreateProduct,
    required this.onUpdateProductPrice,
    required this.onSetProductAvailability,
    required this.onSetProductTaxTreatment,
    required this.onArchiveProduct,
    required this.onReplaceProductImage,
    required this.onDeleteUnusedProduct,
    required this.onCreateProductModifierOption,
    required this.onArchiveProductModifierOption,
    required this.onReviseCustomer,
    required this.onAnonymizeCustomer,
    super.key,
  });

  final CommunityWorkspace workspace;
  final bool isSaving;
  final String applicationSupportDirectory;
  final _CreateCategory onCreateCategory;
  final _ArchiveCategory onArchiveCategory;
  final _CreateProduct onCreateProduct;
  final _UpdateProductPrice onUpdateProductPrice;
  final _SetProductAvailability onSetProductAvailability;
  final _SetProductTaxTreatment onSetProductTaxTreatment;
  final _ArchiveProduct onArchiveProduct;
  final _ReplaceProductImage onReplaceProductImage;
  final _DeleteUnusedProduct onDeleteUnusedProduct;
  final _CreateProductModifierOption onCreateProductModifierOption;
  final _ArchiveProductModifierOption onArchiveProductModifierOption;
  final _ReviseCustomer onReviseCustomer;
  final _AnonymizeCustomer onAnonymizeCustomer;

  @override
  State<_CategoryManager> createState() => _CategoryManagerState();
}

class _CategoryManagerState extends State<_CategoryManager> {
  // The source fence protects the device while Rust decodes untrusted files.
  // It is not the acceptance limit: every accepted image is compressed first.
  static const _maximumSourceImageBytes = 32 * 1024 * 1024;
  static const _maximumCompressedImageBytes = 3 * 1024 * 1024;

  final _formKey = GlobalKey<FormState>();
  final _productFormKey = GlobalKey<FormState>();
  final _categoryController = TextEditingController();
  final _productController = TextEditingController();
  final _priceController = TextEditingController();
  String? _selectedCategoryId;
  String? _selectedBuiltInImageKey;
  Uint8List? _selectedUserImageBytes;
  String? _selectedUserImageName;
  _RemoteMenuImageSelection? _selectedCatalogImage;
  var _hasConfirmedUserImageRights = false;

  @override
  void initState() {
    super.initState();
    _selectDefaultCategoryIfNeeded();
  }

  @override
  void didUpdateWidget(covariant _CategoryManager oldWidget) {
    super.didUpdateWidget(oldWidget);
    _selectDefaultCategoryIfNeeded();
  }

  @override
  void dispose() {
    _categoryController.dispose();
    _productController.dispose();
    _priceController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final isCompact = MediaQuery.sizeOf(context).width < 680;

    return CustomScrollView(
      key: const PageStorageKey('category-manager-scroll'),
      slivers: [
        SliverPadding(
          padding: EdgeInsets.fromLTRB(
            isCompact ? 20 : 44,
            28,
            isCompact ? 20 : 44,
            40,
          ),
          sliver: SliverList(
            delegate: SliverChildListDelegate([
              Text(
                'Menu & products',
                style: Theme.of(context).textTheme.headlineMedium?.copyWith(
                  fontWeight: FontWeight.w800,
                  letterSpacing: -0.7,
                ),
              ),
              const SizedBox(height: 12),
              Align(
                alignment: Alignment.centerLeft,
                child: Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    OutlinedButton.icon(
                      onPressed: () => showModalBottomSheet<void>(
                        context: context,
                        isScrollControlled: true,
                        showDragHandle: true,
                        builder: (_) => _InventoryLedgerSheet(
                          applicationSupportDirectory:
                              widget.applicationSupportDirectory,
                        ),
                      ),
                      icon: const Icon(Icons.inventory_2_outlined),
                      label: const Text('Open stock ledger'),
                    ),
                    OutlinedButton.icon(
                      onPressed: () => showModalBottomSheet<void>(
                        context: context,
                        isScrollControlled: true,
                        showDragHandle: true,
                        builder: (_) => _TaxCatalogSheet(
                          applicationSupportDirectory:
                              widget.applicationSupportDirectory,
                        ),
                      ),
                      icon: const Icon(Icons.percent_outlined),
                      label: const Text('Tax rates'),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 8),
              Text(
                '${widget.workspace.branchName ?? 'Your branch'} • every menu change is saved locally with a durable history.',
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: 22),
              _WorkspaceStatus(status: widget.workspace.storageStatus),
              const SizedBox(height: 22),
              Card(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Form(
                    key: _formKey,
                    child: Wrap(
                      alignment: WrapAlignment.spaceBetween,
                      crossAxisAlignment: WrapCrossAlignment.end,
                      runSpacing: 14,
                      spacing: 14,
                      children: [
                        ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 420),
                          child: TextFormField(
                            controller: _categoryController,
                            enabled: !widget.isSaving,
                            decoration: const InputDecoration(
                              labelText: 'New category',
                              hintText: 'e.g. Hot drinks',
                              prefixIcon: Icon(Icons.category_outlined),
                            ),
                            textCapitalization: TextCapitalization.words,
                            onFieldSubmitted: (_) => _createCategory(),
                            validator: (value) {
                              if (value == null || value.trim().isEmpty) {
                                return 'Enter a category name';
                              }
                              return null;
                            },
                          ),
                        ),
                        FilledButton.icon(
                          onPressed: widget.isSaving ? null : _createCategory,
                          icon: widget.isSaving
                              ? const SizedBox(
                                  height: 18,
                                  width: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.add),
                          label: const Text('Add category'),
                          style: FilledButton.styleFrom(
                            minimumSize: const Size(154, 52),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 18),
              if (widget.workspace.categories.isEmpty)
                _EmptyCategoriesCard()
              else
                ...widget.workspace.categories.map(
                  (category) => Padding(
                    padding: const EdgeInsets.only(bottom: 12),
                    child: Card(
                      child: ListTile(
                        key: ValueKey(category.categoryId),
                        leading: Container(
                          height: 38,
                          width: 38,
                          decoration: BoxDecoration(
                            color: Theme.of(
                              context,
                            ).colorScheme.primaryContainer,
                            borderRadius: const BorderRadius.all(
                              Radius.circular(12),
                            ),
                          ),
                          child: Icon(
                            Icons.restaurant_menu,
                            color: Theme.of(context).colorScheme.primary,
                          ),
                        ),
                        title: Text(
                          category.displayName,
                          style: const TextStyle(fontWeight: FontWeight.w800),
                        ),
                        subtitle: const Text('Active • saved locally'),
                        trailing: IconButton(
                          tooltip: 'Archive category',
                          onPressed: widget.isSaving
                              ? null
                              : () => _archiveCategory(category),
                          icon: const Icon(Icons.archive_outlined),
                        ),
                      ),
                    ),
                  ),
                ),
              const SizedBox(height: 20),
              Text(
                'Menu items',
                style: Theme.of(
                  context,
                ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
              ),
              const SizedBox(height: 6),
              Text(
                'Use exact rupee-and-paise pricing. Prices are stored as integer minor units, never floating point.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: 14),
              if (widget.workspace.categories.isEmpty)
                const _ProductSetupHint()
              else
                _ProductComposer(
                  formKey: _productFormKey,
                  productController: _productController,
                  priceController: _priceController,
                  categories: widget.workspace.categories,
                  selectedCategoryId: _selectedCategoryId,
                  isSaving: widget.isSaving,
                  selectedBuiltInImageKey: _selectedBuiltInImageKey,
                  selectedUserImageBytes: _selectedUserImageBytes,
                  selectedUserImageName: _selectedUserImageName,
                  selectedCatalogImageName: _selectedCatalogImage?.displayName,
                  hasConfirmedUserImageRights: _hasConfirmedUserImageRights,
                  onCategoryChanged: (categoryId) {
                    setState(() {
                      _selectedCategoryId = categoryId;
                    });
                  },
                  onBuiltInImageSelected: _selectBuiltInImage,
                  onUserImageRequested: _pickUserImage,
                  onCatalogImageRequested: _pickCatalogImage,
                  onImageCleared: _clearSelectedImage,
                  onUserImageRightsChanged: (value) {
                    setState(() {
                      _hasConfirmedUserImageRights = value ?? false;
                    });
                  },
                  onSubmit: _createProduct,
                ),
              const SizedBox(height: 18),
              _ProductList(
                products: widget.workspace.products,
                categories: widget.workspace.categories,
                isSaving: widget.isSaving,
                onUpdateProductPrice: widget.onUpdateProductPrice,
                onSetProductAvailability: widget.onSetProductAvailability,
                onSetProductTaxTreatment: widget.onSetProductTaxTreatment,
                onArchiveProduct: widget.onArchiveProduct,
                onReplaceProductImage: widget.onReplaceProductImage,
                onDeleteUnusedProduct: widget.onDeleteUnusedProduct,
                onCreateProductModifierOption:
                    widget.onCreateProductModifierOption,
                onArchiveProductModifierOption:
                    widget.onArchiveProductModifierOption,
              ),
              const SizedBox(height: 28),
              _CustomerPrivacyList(
                customers: widget.workspace.customers,
                isSaving: widget.isSaving,
                onReviseCustomer: widget.onReviseCustomer,
                onAnonymizeCustomer: widget.onAnonymizeCustomer,
              ),
            ]),
          ),
        ),
      ],
    );
  }

  Future<void> _createCategory() async {
    if (!(_formKey.currentState?.validate() ?? false)) {
      return;
    }

    final previousCount = widget.workspace.categories.length;
    await widget.onCreateCategory(_categoryController.text);
    if (mounted && widget.workspace.categories.length > previousCount) {
      _categoryController.clear();
    }
  }

  void _selectDefaultCategoryIfNeeded() {
    final selectedCategoryExists = widget.workspace.categories.any(
      (category) => category.categoryId == _selectedCategoryId,
    );
    if (!selectedCategoryExists) {
      _selectedCategoryId = widget.workspace.categories.isEmpty
          ? null
          : widget.workspace.categories.first.categoryId;
    }
  }

  Future<void> _archiveCategory(CommunityCategoryView category) async {
    final reasonController = TextEditingController();
    try {
      final reason = await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: Text('Archive ${category.displayName}?'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Text(
                'Active products must be removed first. Category history stays retained.',
              ),
              const SizedBox(height: 12),
              TextField(
                controller: reasonController,
                maxLength: 500,
                autofocus: true,
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
            FilledButton(
              onPressed: () {
                final value = reasonController.text.trim();
                if (value.length < 3) return;
                Navigator.of(dialogContext).pop(value);
              },
              child: const Text('Archive category'),
            ),
          ],
        ),
      );
      if (reason == null) return;
      await widget.onArchiveCategory(
        categoryId: category.categoryId,
        expectedRevision: category.revision,
        reason: reason,
      );
    } finally {
      reasonController.dispose();
    }
  }

  Future<void> _createProduct() async {
    if (!(_productFormKey.currentState?.validate() ?? false)) {
      return;
    }
    if (_selectedUserImageBytes != null &&
        _selectedCatalogImage == null &&
        !_hasConfirmedUserImageRights) {
      _showImageMessage(
        'Confirm that your restaurant can use the selected image before adding this menu item.',
      );
      return;
    }

    final unitPriceMinor = parseInrPriceToMinorUnits(_priceController.text);
    final categoryId = _selectedCategoryId;
    if (unitPriceMinor == null || categoryId == null) {
      return;
    }

    final previousCount = widget.workspace.products.length;
    await widget.onCreateProduct(
      displayName: _productController.text,
      categoryId: categoryId,
      unitPriceMinor: unitPriceMinor,
      builtInImageKey: _selectedBuiltInImageKey,
      userImageBytes: _selectedUserImageBytes,
      catalogImage: _selectedCatalogImage,
    );
    if (mounted && widget.workspace.products.length > previousCount) {
      _productController.clear();
      _priceController.clear();
      _clearSelectedImage();
    }
  }

  Future<void> _pickUserImage() async {
    if (widget.isSaving) {
      return;
    }

    try {
      final result = await FilePicker.pickFiles(
        allowMultiple: false,
        type: FileType.custom,
        allowedExtensions: const ['jpg', 'jpeg', 'png', 'webp'],
        withData: true,
      );
      if (!mounted || result == null) {
        return;
      }

      final file = result.files.single;
      final bytes = file.bytes;
      if (bytes == null || bytes.isEmpty) {
        _showImageMessage(
          'That image could not be read. Please choose a PNG, JPEG, or WebP image.',
        );
        return;
      }
      if (bytes.lengthInBytes > _maximumSourceImageBytes) {
        _showImageMessage(
          'This image is too large to prepare safely. Choose a file below 32 MB.',
        );
        return;
      }
      final preparedBytes = await _prepareMenuImage(bytes);
      if (!mounted || preparedBytes == null) {
        return;
      }

      setState(() {
        _selectedBuiltInImageKey = null;
        _selectedUserImageBytes = preparedBytes;
        _selectedUserImageName = file.name;
        _selectedCatalogImage = null;
        _hasConfirmedUserImageRights = false;
      });
    } catch (_) {
      if (mounted) {
        _showImageMessage(
          'The image picker could not open. Please try again or choose a different image.',
        );
      }
    }
  }

  void _selectBuiltInImage(String imageKey) {
    setState(() {
      _selectedBuiltInImageKey = imageKey;
      _selectedUserImageBytes = null;
      _selectedUserImageName = null;
      _selectedCatalogImage = null;
      _hasConfirmedUserImageRights = false;
    });
  }

  Future<void> _pickCatalogImage() async {
    if (widget.isSaving) {
      return;
    }
    final selection = await showModalBottomSheet<_RemoteMenuImageSelection>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => const _RemoteMenuImageChooser(),
    );
    if (!mounted || selection == null) {
      return;
    }
    final preparedBytes = await _prepareMenuImage(selection.bytes);
    if (!mounted || preparedBytes == null) {
      return;
    }
    setState(() {
      _selectedBuiltInImageKey = null;
      _selectedUserImageBytes = preparedBytes;
      _selectedUserImageName = selection.displayName;
      _selectedCatalogImage = selection;
      _hasConfirmedUserImageRights = false;
    });
  }

  Future<Uint8List?> _prepareMenuImage(Uint8List sourceBytes) async {
    try {
      final preparedBytes = await prepareCommunityMenuImage(
        imageBytes: sourceBytes,
      );
      if (preparedBytes.isEmpty ||
          preparedBytes.lengthInBytes > _maximumCompressedImageBytes) {
        _showImageMessage(
          'This image could not be compressed below the 3 MB menu-image limit. Choose a simpler image.',
        );
        return null;
      }
      return preparedBytes;
    } catch (_) {
      _showImageMessage(
        'This image could not be prepared below the 3 MB menu-image limit. Choose a JPEG, PNG, or WebP image.',
      );
      return null;
    }
  }

  void _clearSelectedImage() {
    setState(() {
      _selectedBuiltInImageKey = null;
      _selectedUserImageBytes = null;
      _selectedUserImageName = null;
      _selectedCatalogImage = null;
      _hasConfirmedUserImageRights = false;
    });
  }

  void _showImageMessage(String message) {
    if (!mounted) {
      return;
    }
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }
}

class _CustomerPrivacyList extends StatelessWidget {
  const _CustomerPrivacyList({
    required this.customers,
    required this.isSaving,
    required this.onReviseCustomer,
    required this.onAnonymizeCustomer,
  });

  final List<CommunityCustomerView> customers;
  final bool isSaving;
  final _ReviseCustomer onReviseCustomer;
  final _AnonymizeCustomer onAnonymizeCustomer;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Customers & privacy',
          style: Theme.of(
            context,
          ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
        ),
        const SizedBox(height: 6),
        Text(
          'Corrections and anonymization are retained as accountable history; invoices are never deleted.',
          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 14),
        if (customers.isEmpty)
          const Card(
            child: ListTile(
              leading: Icon(Icons.person_outline),
              title: Text('No active customers yet'),
              subtitle: Text(
                'Cashier, manager, or owner can add one from the counter.',
              ),
            ),
          )
        else
          ...customers.map(
            (customer) => Card(
              child: ListTile(
                leading: const CircleAvatar(child: Icon(Icons.person_outline)),
                title: Text(customer.displayName),
                subtitle: Text(
                  [
                    if (customer.phoneNumber != null) customer.phoneNumber!,
                    if (customer.emailAddress != null) customer.emailAddress!,
                    customer.marketingConsent
                        ? 'Marketing consent recorded'
                        : 'No marketing consent',
                  ].join(' • '),
                ),
                trailing: PopupMenuButton<String>(
                  enabled: !isSaving,
                  onSelected: (action) async {
                    if (action == 'correct') {
                      final request = await _requestCustomerCorrection(
                        context,
                        customer,
                      );
                      if (request == null || !context.mounted) return;
                      await onReviseCustomer(
                        customerId: customer.customerId,
                        displayName: request.displayName,
                        phoneNumber: request.phoneNumber,
                        emailAddress: request.emailAddress,
                        marketingConsent: request.marketingConsent,
                        reason: request.reason,
                      );
                    } else {
                      final reason = await _requestCustomerAnonymization(
                        context,
                        customer,
                      );
                      if (reason == null || !context.mounted) return;
                      await onAnonymizeCustomer(
                        customerId: customer.customerId,
                        reason: reason,
                      );
                    }
                  },
                  itemBuilder: (context) => const [
                    PopupMenuItem(
                      value: 'correct',
                      child: ListTile(
                        leading: Icon(Icons.edit_outlined),
                        title: Text('Correct profile'),
                      ),
                    ),
                    PopupMenuItem(
                      value: 'anonymize',
                      child: ListTile(
                        leading: Icon(Icons.person_off_outlined),
                        title: Text('Anonymize customer'),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

class _CustomerCorrectionRequest {
  const _CustomerCorrectionRequest({
    required this.displayName,
    required this.phoneNumber,
    required this.emailAddress,
    required this.marketingConsent,
    required this.reason,
  });

  final String displayName;
  final String? phoneNumber;
  final String? emailAddress;
  final bool marketingConsent;
  final String reason;
}

Future<_CustomerCorrectionRequest?> _requestCustomerCorrection(
  BuildContext context,
  CommunityCustomerView customer,
) async {
  final nameController = TextEditingController(text: customer.displayName);
  final phoneController = TextEditingController(
    text: customer.phoneNumber ?? '',
  );
  final emailController = TextEditingController(
    text: customer.emailAddress ?? '',
  );
  final reasonController = TextEditingController();
  var marketingConsent = customer.marketingConsent;
  try {
    return await showDialog<_CustomerCorrectionRequest>(
      context: context,
      builder: (dialogContext) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const Text('Correct customer profile'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  controller: nameController,
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
                ),
                TextField(
                  controller: reasonController,
                  maxLength: 280,
                  textCapitalization: TextCapitalization.sentences,
                  decoration: const InputDecoration(
                    labelText: 'Correction reason *',
                    helperText:
                        'A new immutable profile revision will be recorded.',
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
              onPressed: () {
                final name = nameController.text.trim();
                final reason = reasonController.text.trim();
                if (name.isEmpty || reason.length < 3) return;
                Navigator.of(dialogContext).pop(
                  _CustomerCorrectionRequest(
                    displayName: name,
                    phoneNumber: _optionalCustomerText(phoneController.text),
                    emailAddress: _optionalCustomerText(emailController.text),
                    marketingConsent: marketingConsent,
                    reason: reason,
                  ),
                );
              },
              child: const Text('Save correction'),
            ),
          ],
        ),
      ),
    );
  } finally {
    nameController.dispose();
    phoneController.dispose();
    emailController.dispose();
    reasonController.dispose();
  }
}

Future<String?> _requestCustomerAnonymization(
  BuildContext context,
  CommunityCustomerView customer,
) async {
  final reasonController = TextEditingController();
  try {
    return await showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text('Anonymize ${customer.displayName}?'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text(
              'Contact details and marketing consent will be redacted in a new immutable profile revision. Existing invoices and audit history remain.',
            ),
            const SizedBox(height: 12),
            TextField(
              controller: reasonController,
              maxLength: 280,
              autofocus: true,
              textCapitalization: TextCapitalization.sentences,
              decoration: const InputDecoration(labelText: 'Reason *'),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton.tonalIcon(
            onPressed: () {
              final reason = reasonController.text.trim();
              if (reason.length >= 3) Navigator.of(dialogContext).pop(reason);
            },
            icon: const Icon(Icons.person_off_outlined),
            label: const Text('Anonymize'),
          ),
        ],
      ),
    );
  } finally {
    reasonController.dispose();
  }
}

String? _optionalCustomerText(String value) {
  final trimmed = value.trim();
  return trimmed.isEmpty ? null : trimmed;
}

String _taxTreatmentLabel(String treatment) {
  return switch (treatment) {
    'exclusive' => 'exclusive tax',
    'inclusive' => 'inclusive tax',
    _ => 'no tax',
  };
}

class _TaxCatalogSheet extends StatefulWidget {
  const _TaxCatalogSheet({required this.applicationSupportDirectory});

  final String applicationSupportDirectory;

  @override
  State<_TaxCatalogSheet> createState() => _TaxCatalogSheetState();
}

class _TaxCatalogSheetState extends State<_TaxCatalogSheet> {
  late Future<CommunityTaxRateWorkspace> _rates;

  @override
  void initState() {
    super.initState();
    _rates = _load();
  }

  Future<CommunityTaxRateWorkspace> _load() => listCommunityTaxRates(
    applicationSupportDirectory: widget.applicationSupportDirectory,
  );

  Future<void> _createRate() async {
    final nameController = TextEditingController();
    final percentController = TextEditingController();
    try {
      final created = await showDialog<bool>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: const Text('Add tax rate'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Text(
                'Named branch rates are provider-neutral. Active exclusive or inclusive products apply every active rate. This is not a GST filing claim.',
              ),
              const SizedBox(height: 12),
              TextField(
                controller: nameController,
                textCapitalization: TextCapitalization.words,
                decoration: const InputDecoration(labelText: 'Display name'),
              ),
              const SizedBox(height: 8),
              TextField(
                controller: percentController,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                inputFormatters: [
                  FilteringTextInputFormatter.allow(RegExp(r'[0-9.]')),
                ],
                decoration: const InputDecoration(
                  labelText: 'Rate percent',
                  helperText: 'Example: 5 for 5%',
                ),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(false),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                final name = nameController.text.trim();
                final percent = double.tryParse(percentController.text.trim());
                if (name.isEmpty ||
                    percent == null ||
                    percent < 0 ||
                    percent > 100) {
                  return;
                }
                Navigator.of(dialogContext).pop(true);
              },
              child: const Text('Save rate'),
            ),
          ],
        ),
      );
      if (created != true || !mounted) {
        return;
      }
      final name = nameController.text.trim();
      final percent = double.parse(percentController.text.trim());
      final basisPoints = (percent * 100).round();
      final workspace = await createCommunityTaxRate(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        displayName: name,
        basisPoints: basisPoints,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _rates = Future.value(workspace);
      });
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(workspace.storageStatus)));
    } finally {
      nameController.dispose();
      percentController.dispose();
    }
  }

  Future<void> _archiveRate(CommunityTaxRateView rate) async {
    final reasonController = TextEditingController();
    try {
      final reason = await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: Text('Archive ${rate.displayName}?'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Text(
                'Archived rates stop applying to new exclusive or inclusive sales. History stays retained. This is not a GST filing change.',
              ),
              const SizedBox(height: 12),
              TextField(
                controller: reasonController,
                maxLength: 500,
                autofocus: true,
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
            FilledButton(
              onPressed: () {
                final value = reasonController.text.trim();
                if (value.length < 3) {
                  return;
                }
                Navigator.of(dialogContext).pop(value);
              },
              child: const Text('Archive rate'),
            ),
          ],
        ),
      );
      if (reason == null || !mounted) {
        return;
      }
      final workspace = await archiveCommunityTaxRate(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        taxRateId: rate.taxRateId,
        expectedRevision: rate.revision,
        reason: reason,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _rates = Future.value(workspace);
      });
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(workspace.storageStatus)));
    } finally {
      reasonController.dispose();
    }
  }

  @override
  Widget build(BuildContext context) {
    final bottomInset = MediaQuery.viewInsetsOf(context).bottom;
    return Padding(
      padding: EdgeInsets.only(bottom: bottomInset),
      child: SafeArea(
        child: SizedBox(
          height: MediaQuery.sizeOf(context).height * 0.72,
          child: FutureBuilder<CommunityTaxRateWorkspace>(
            future: _rates,
            builder: (context, snapshot) {
              final workspace = snapshot.data;
              final rates =
                  workspace?.rates.where((rate) => !rate.archived).toList() ??
                  const <CommunityTaxRateView>[];
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(24, 8, 24, 0),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            'Branch tax rates',
                            style: Theme.of(context).textTheme.titleLarge
                                ?.copyWith(fontWeight: FontWeight.w800),
                          ),
                        ),
                        FilledButton.tonalIcon(
                          onPressed: _createRate,
                          icon: const Icon(Icons.add),
                          label: const Text('Add rate'),
                        ),
                      ],
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(24, 8, 24, 12),
                    child: Text(
                      workspace?.storageStatus ??
                          (snapshot.hasError
                              ? 'Tax rates need attention • local storage could not be read.'
                              : 'Loading tax rates…'),
                      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                        color: Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                  Expanded(
                    child: rates.isEmpty
                        ? const Center(
                            child: Padding(
                              padding: EdgeInsets.all(24),
                              child: Text(
                                'No active tax rates yet. Products marked exclusive or inclusive need at least one active rate before taxed sales can complete.',
                                textAlign: TextAlign.center,
                              ),
                            ),
                          )
                        : ListView.separated(
                            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
                            itemCount: rates.length,
                            separatorBuilder: (_, _) =>
                                const SizedBox(height: 8),
                            itemBuilder: (context, index) {
                              final rate = rates[index];
                              final percent = (rate.basisPoints / 100)
                                  .toStringAsFixed(
                                    rate.basisPoints % 100 == 0 ? 0 : 2,
                                  );
                              return Card(
                                child: ListTile(
                                  title: Text(
                                    rate.displayName,
                                    style: const TextStyle(
                                      fontWeight: FontWeight.w700,
                                    ),
                                  ),
                                  subtitle: Text('$percent% • saved locally'),
                                  trailing: Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      Text(
                                        '${rate.basisPoints} bp',
                                        style: Theme.of(context)
                                            .textTheme
                                            .labelLarge
                                            ?.copyWith(
                                              fontWeight: FontWeight.w700,
                                            ),
                                      ),
                                      IconButton(
                                        tooltip: 'Archive rate',
                                        onPressed: () => _archiveRate(rate),
                                        icon: const Icon(
                                          Icons.archive_outlined,
                                        ),
                                      ),
                                    ],
                                  ),
                                ),
                              );
                            },
                          ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}

class _InventoryLedgerSheet extends StatefulWidget {
  const _InventoryLedgerSheet({required this.applicationSupportDirectory});

  final String applicationSupportDirectory;

  @override
  State<_InventoryLedgerSheet> createState() => _InventoryLedgerSheetState();
}

class _InventoryLedgerSheetState extends State<_InventoryLedgerSheet> {
  late Future<CommunityInventoryWorkspace> _inventory;

  @override
  void initState() {
    super.initState();
    _inventory = _load();
  }

  Future<CommunityInventoryWorkspace> _load() => loadCommunityInventory(
    applicationSupportDirectory: widget.applicationSupportDirectory,
  );

  Future<void> _recordMovement(CommunityInventoryItemView item) async {
    final quantityController = TextEditingController();
    final reasonController = TextEditingController();
    var movementType = item.tracked ? 'purchase' : 'opening';
    final request = await showDialog<_InventoryMovementRequest>(
      context: context,
      builder: (dialogContext) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: Text('Stock movement — ${item.displayName}'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                DropdownButtonFormField<String>(
                  initialValue: movementType,
                  decoration: const InputDecoration(labelText: 'Movement'),
                  items: const [
                    DropdownMenuItem(
                      value: 'opening',
                      child: Text('Opening stock'),
                    ),
                    DropdownMenuItem(
                      value: 'purchase',
                      child: Text('Purchase received'),
                    ),
                    DropdownMenuItem(
                      value: 'waste',
                      child: Text('Waste / spoilage'),
                    ),
                    DropdownMenuItem(
                      value: 'adjustment',
                      child: Text('Count adjustment'),
                    ),
                  ],
                  onChanged: (value) => setDialogState(() {
                    movementType = value ?? movementType;
                  }),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: quantityController,
                  autofocus: true,
                  keyboardType: TextInputType.number,
                  decoration: InputDecoration(
                    labelText: movementType == 'adjustment'
                        ? 'Signed quantity change'
                        : 'Quantity',
                    helperText: movementType == 'adjustment'
                        ? 'Use a negative number to reduce counted stock.'
                        : 'Use a positive whole number.',
                  ),
                ),
                if (movementType == 'waste' ||
                    movementType == 'adjustment') ...[
                  const SizedBox(height: 12),
                  TextField(
                    controller: reasonController,
                    maxLength: 500,
                    textCapitalization: TextCapitalization.sentences,
                    decoration: const InputDecoration(
                      labelText: 'Reason',
                      helperText: 'This cannot be edited or deleted later.',
                    ),
                  ),
                ],
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                final quantity = int.tryParse(quantityController.text.trim());
                final requiresReason =
                    movementType == 'waste' || movementType == 'adjustment';
                if (quantity == null ||
                    quantity == 0 ||
                    (movementType != 'adjustment' && quantity < 0) ||
                    (requiresReason &&
                        reasonController.text.trim().length < 3)) {
                  return;
                }
                Navigator.of(dialogContext).pop(
                  _InventoryMovementRequest(
                    movementType: movementType,
                    quantity: quantity,
                    reason: requiresReason
                        ? reasonController.text.trim()
                        : null,
                  ),
                );
              },
              child: const Text('Record movement'),
            ),
          ],
        ),
      ),
    );
    if (!mounted || request == null) return;
    final inventory = await recordCommunityInventoryMovement(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      productId: item.productId,
      movementType: request.movementType,
      quantity: request.quantity,
      reason: request.reason,
    );
    if (!mounted) return;
    setState(() => _inventory = Future.value(inventory));
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(inventory.storageStatus)));
  }

  Future<void> _setLowStockThreshold(CommunityInventoryItemView item) async {
    if (!item.tracked) return;
    final thresholdController = TextEditingController(
      text: item.lowStockThreshold?.toString() ?? '',
    );
    final reasonController = TextEditingController();
    final request = await showDialog<_LowStockThresholdRequest>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text('Low-stock threshold — ${item.displayName}'),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: thresholdController,
                autofocus: true,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(
                  labelText: 'Alert at or below (units)',
                  helperText: 'Use 0 to alert only when stock is empty.',
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: reasonController,
                maxLength: 500,
                minLines: 2,
                maxLines: 4,
                textCapitalization: TextCapitalization.sentences,
                decoration: const InputDecoration(
                  labelText: 'Reason',
                  helperText: 'Policy changes are retained in history.',
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
            onPressed: () {
              final threshold = int.tryParse(thresholdController.text.trim());
              if (threshold == null ||
                  threshold < 0 ||
                  reasonController.text.trim().length < 3) {
                return;
              }
              Navigator.of(dialogContext).pop(
                _LowStockThresholdRequest(
                  threshold: threshold,
                  reason: reasonController.text.trim(),
                ),
              );
            },
            child: const Text('Save threshold'),
          ),
        ],
      ),
    );
    if (!mounted || request == null) return;
    final inventory = await setCommunityInventoryLowStockThreshold(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      productId: item.productId,
      thresholdQuantity: request.threshold,
      reason: request.reason,
    );
    if (!mounted) return;
    setState(() => _inventory = Future.value(inventory));
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(inventory.storageStatus)));
  }

  Future<void> _clearLowStockThreshold(CommunityInventoryItemView item) async {
    if (!item.tracked || item.lowStockThreshold == null) return;
    final reasonController = TextEditingController();
    final reason = await showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text('Clear low-stock alert — ${item.displayName}'),
        content: TextField(
          controller: reasonController,
          autofocus: true,
          maxLength: 500,
          minLines: 2,
          maxLines: 4,
          textCapitalization: TextCapitalization.sentences,
          decoration: const InputDecoration(
            labelText: 'Reason',
            helperText: 'The prior threshold and this change stay in history.',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () {
              final value = reasonController.text.trim();
              if (value.length >= 3) {
                Navigator.of(dialogContext).pop(value);
              }
            },
            child: const Text('Clear alert'),
          ),
        ],
      ),
    );
    if (!mounted || reason == null) return;
    final inventory = await clearCommunityInventoryLowStockThreshold(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      productId: item.productId,
      reason: reason,
    );
    if (!mounted) return;
    setState(() => _inventory = Future.value(inventory));
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(inventory.storageStatus)));
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.82,
        child: FutureBuilder<CommunityInventoryWorkspace>(
          future: _inventory,
          builder: (context, snapshot) {
            if (snapshot.connectionState != ConnectionState.done) {
              return const Center(child: CircularProgressIndicator());
            }
            final inventory = snapshot.data;
            if (inventory == null || !inventory.available) {
              return _FeatureCanvas(
                icon: Icons.inventory_2_outlined,
                title: 'Inventory needs attention',
                description:
                    inventory?.storageStatus ??
                    'The local stock ledger could not be loaded.',
              );
            }
            return ListView(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 28),
              children: [
                Text(
                  'Stock ledger',
                  style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
                const SizedBox(height: 6),
                Text(
                  'Balances are derived from immutable movements. Configure optional low-stock alerts without changing stock. Zero-stock tracked items cannot be sold.',
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
                const SizedBox(height: 14),
                _WorkspaceStatus(status: inventory.storageStatus),
                const SizedBox(height: 14),
                if (inventory.items.isEmpty)
                  const Card(
                    child: Padding(
                      padding: EdgeInsets.all(20),
                      child: Text('Add a menu item before recording stock.'),
                    ),
                  ),
                for (final item in inventory.items) ...[
                  Card(
                    child: ListTile(
                      leading: Icon(
                        item.tracked
                            ? Icons.inventory_2_outlined
                            : Icons.inventory_2,
                      ),
                      title: Text(item.displayName),
                      subtitle: Text(
                        item.tracked
                            ? '${item.balance} units in stock • ${item.lowStock ? 'low stock' : 'tracked'}${item.lowStockThreshold == null ? '' : ' • alert at ${item.lowStockThreshold}'}'
                            : 'Not stock-tracked',
                      ),
                      trailing: Wrap(
                        spacing: 4,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          if (item.lowStock)
                            Icon(
                              Icons.warning_amber_rounded,
                              color: Theme.of(context).colorScheme.error,
                            ),
                          TextButton(
                            onPressed: () => _recordMovement(item),
                            child: const Text('Record'),
                          ),
                          IconButton(
                            tooltip: 'Set low-stock threshold',
                            onPressed: item.tracked
                                ? () => _setLowStockThreshold(item)
                                : null,
                            icon: const Icon(
                              Icons.notifications_active_outlined,
                            ),
                          ),
                          if (item.lowStockThreshold != null)
                            IconButton(
                              tooltip: 'Clear low-stock threshold',
                              onPressed: () => _clearLowStockThreshold(item),
                              icon: const Icon(
                                Icons.notifications_off_outlined,
                              ),
                            ),
                        ],
                      ),
                    ),
                  ),
                  const SizedBox(height: 8),
                ],
              ],
            );
          },
        ),
      ),
    );
  }
}

class _InventoryMovementRequest {
  const _InventoryMovementRequest({
    required this.movementType,
    required this.quantity,
    required this.reason,
  });

  final String movementType;
  final int quantity;
  final String? reason;
}

class _LowStockThresholdRequest {
  const _LowStockThresholdRequest({
    required this.threshold,
    required this.reason,
  });

  final int threshold;
  final String reason;
}

class _ProductComposer extends StatelessWidget {
  const _ProductComposer({
    required this.formKey,
    required this.productController,
    required this.priceController,
    required this.categories,
    required this.selectedCategoryId,
    required this.isSaving,
    required this.selectedBuiltInImageKey,
    required this.selectedUserImageBytes,
    required this.selectedUserImageName,
    required this.selectedCatalogImageName,
    required this.hasConfirmedUserImageRights,
    required this.onCategoryChanged,
    required this.onBuiltInImageSelected,
    required this.onUserImageRequested,
    required this.onCatalogImageRequested,
    required this.onImageCleared,
    required this.onUserImageRightsChanged,
    required this.onSubmit,
  });

  final GlobalKey<FormState> formKey;
  final TextEditingController productController;
  final TextEditingController priceController;
  final List<CommunityCategoryView> categories;
  final String? selectedCategoryId;
  final bool isSaving;
  final String? selectedBuiltInImageKey;
  final Uint8List? selectedUserImageBytes;
  final String? selectedUserImageName;
  final String? selectedCatalogImageName;
  final bool hasConfirmedUserImageRights;
  final ValueChanged<String?> onCategoryChanged;
  final ValueChanged<String> onBuiltInImageSelected;
  final Future<void> Function() onUserImageRequested;
  final Future<void> Function() onCatalogImageRequested;
  final VoidCallback onImageCleared;
  final ValueChanged<bool?> onUserImageRightsChanged;
  final VoidCallback onSubmit;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Form(
          key: formKey,
          child: LayoutBuilder(
            builder: (context, constraints) {
              final compact = constraints.maxWidth < 640;
              final fields = [
                TextFormField(
                  controller: productController,
                  enabled: !isSaving,
                  decoration: const InputDecoration(
                    labelText: 'Menu item',
                    hintText: 'e.g. Masala chai',
                    prefixIcon: Icon(Icons.restaurant_outlined),
                  ),
                  textCapitalization: TextCapitalization.words,
                  onFieldSubmitted: (_) => onSubmit(),
                  validator: (value) {
                    if (value == null || value.trim().isEmpty) {
                      return 'Enter a menu item name';
                    }
                    return null;
                  },
                ),
                TextFormField(
                  controller: priceController,
                  enabled: !isSaving,
                  decoration: const InputDecoration(
                    labelText: 'Price (INR)',
                    hintText: 'e.g. 125.00',
                    prefixIcon: Icon(Icons.currency_rupee),
                  ),
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  onFieldSubmitted: (_) => onSubmit(),
                  validator: (value) {
                    if (parseInrPriceToMinorUnits(value ?? '') == null) {
                      return 'Use a non-negative amount with up to two decimals';
                    }
                    return null;
                  },
                ),
                DropdownButtonFormField<String>(
                  initialValue: selectedCategoryId,
                  isExpanded: true,
                  decoration: const InputDecoration(
                    labelText: 'Category',
                    prefixIcon: Icon(Icons.category_outlined),
                  ),
                  onChanged: isSaving ? null : onCategoryChanged,
                  items: categories
                      .map(
                        (category) => DropdownMenuItem(
                          value: category.categoryId,
                          child: Text(
                            category.displayName,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      )
                      .toList(growable: false),
                  validator: (value) => value == null
                      ? 'Choose the category for this menu item'
                      : null,
                ),
              ];

              final submit = FilledButton.icon(
                onPressed: isSaving ? null : onSubmit,
                icon: isSaving
                    ? const SizedBox(
                        height: 18,
                        width: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.add),
                label: const Text('Add menu item'),
                style: FilledButton.styleFrom(minimumSize: const Size(164, 52)),
              );

              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (compact) ...[
                    ...fields.expand(
                      (field) => [field, const SizedBox(height: 14)],
                    ),
                    submit,
                  ] else
                    Wrap(
                      alignment: WrapAlignment.spaceBetween,
                      crossAxisAlignment: WrapCrossAlignment.end,
                      runSpacing: 14,
                      spacing: 14,
                      children: [
                        for (final field in fields)
                          ConstrainedBox(
                            constraints: const BoxConstraints(maxWidth: 300),
                            child: field,
                          ),
                        submit,
                      ],
                    ),
                  const Padding(
                    padding: EdgeInsets.symmetric(vertical: 20),
                    child: Divider(),
                  ),
                  _MenuItemImagePicker(
                    builtInImageKey: selectedBuiltInImageKey,
                    userImageBytes: selectedUserImageBytes,
                    userImageName: selectedUserImageName,
                    catalogImageName: selectedCatalogImageName,
                    hasConfirmedUserImageRights: hasConfirmedUserImageRights,
                    enabled: !isSaving,
                    onBuiltInImageSelected: onBuiltInImageSelected,
                    onUserImageRequested: onUserImageRequested,
                    onCatalogImageRequested: onCatalogImageRequested,
                    onImageCleared: onImageCleared,
                    onUserImageRightsChanged: onUserImageRightsChanged,
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}

class _MenuItemImagePicker extends StatelessWidget {
  const _MenuItemImagePicker({
    required this.builtInImageKey,
    required this.userImageBytes,
    required this.userImageName,
    required this.catalogImageName,
    required this.hasConfirmedUserImageRights,
    required this.enabled,
    required this.onBuiltInImageSelected,
    required this.onUserImageRequested,
    required this.onCatalogImageRequested,
    required this.onImageCleared,
    required this.onUserImageRightsChanged,
  });

  final String? builtInImageKey;
  final Uint8List? userImageBytes;
  final String? userImageName;
  final String? catalogImageName;
  final bool hasConfirmedUserImageRights;
  final bool enabled;
  final ValueChanged<String> onBuiltInImageSelected;
  final Future<void> Function() onUserImageRequested;
  final Future<void> Function() onCatalogImageRequested;
  final VoidCallback onImageCleared;
  final ValueChanged<bool?> onUserImageRightsChanged;

  bool get _hasSelection => builtInImageKey != null || userImageBytes != null;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final isUserImage = userImageBytes != null && catalogImageName == null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Menu photo (optional)',
          style: Theme.of(
            context,
          ).textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w800),
        ),
        const SizedBox(height: 4),
        Text(
          'Choose an app photo, a verified Gotigin catalogue image, or a restaurant-owned image. Every custom image is compressed locally and accepted only when it fits the 3 MB menu-image limit.',
          style: Theme.of(
            context,
          ).textTheme.bodySmall?.copyWith(color: colorScheme.onSurfaceVariant),
        ),
        const SizedBox(height: 14),
        if (_hasSelection) ...[
          _MenuItemImagePreview(
            builtInImageKey: builtInImageKey,
            userImageBytes: userImageBytes,
            userImageName: userImageName,
            catalogImageName: catalogImageName,
          ),
          const SizedBox(height: 12),
        ],
        Wrap(
          spacing: 12,
          runSpacing: 10,
          children: [
            OutlinedButton.icon(
              onPressed: enabled ? () => onCatalogImageRequested() : null,
              icon: const Icon(Icons.cloud_outlined),
              label: const Text('Search Gotigin photos'),
            ),
            OutlinedButton.icon(
              onPressed: enabled
                  ? () => _showBuiltInImageChooser(context)
                  : null,
              icon: const Icon(Icons.auto_awesome_mosaic_outlined),
              label: Text(
                _hasSelection ? 'Change app photo' : 'Choose app photo',
              ),
            ),
            OutlinedButton.icon(
              onPressed: enabled ? onUserImageRequested : null,
              icon: const Icon(Icons.add_photo_alternate_outlined),
              label: Text(
                _hasSelection ? 'Use a different image' : 'Use my image',
              ),
            ),
            if (_hasSelection)
              TextButton.icon(
                onPressed: enabled ? onImageCleared : null,
                icon: const Icon(Icons.close),
                label: const Text('Remove photo'),
              ),
          ],
        ),
        const SizedBox(height: 8),
        Text(
          'Use only images your restaurant owns or is licensed to use.',
          style: Theme.of(
            context,
          ).textTheme.bodySmall?.copyWith(color: colorScheme.onSurfaceVariant),
        ),
        if (isUserImage)
          CheckboxListTile(
            value: hasConfirmedUserImageRights,
            onChanged: enabled ? onUserImageRightsChanged : null,
            contentPadding: EdgeInsets.zero,
            controlAffinity: ListTileControlAffinity.leading,
            title: const Text(
              'I confirm that this restaurant owns or is licensed to use this image.',
            ),
          ),
      ],
    );
  }

  Future<void> _showBuiltInImageChooser(BuildContext context) async {
    final imageKey = await showModalBottomSheet<String>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => const _BuiltInImageChooser(),
    );
    if (imageKey != null) {
      onBuiltInImageSelected(imageKey);
    }
  }
}

class _MenuItemImagePreview extends StatelessWidget {
  const _MenuItemImagePreview({
    required this.builtInImageKey,
    required this.userImageBytes,
    required this.userImageName,
    required this.catalogImageName,
  });

  final String? builtInImageKey;
  final Uint8List? userImageBytes;
  final String? userImageName;
  final String? catalogImageName;

  @override
  Widget build(BuildContext context) {
    final option = _builtInImageForKey(builtInImageKey);
    final colorScheme = Theme.of(context).colorScheme;
    final isCatalogImage = catalogImageName != null;
    final isUserImage = userImageBytes != null && !isCatalogImage;

    return Container(
      constraints: const BoxConstraints(minHeight: 96),
      decoration: BoxDecoration(
        color: colorScheme.surfaceContainerHighest,
        borderRadius: const BorderRadius.all(Radius.circular(16)),
        border: Border.all(color: colorScheme.outlineVariant),
      ),
      child: Row(
        children: [
          SizedBox(
            height: 96,
            width: 128,
            child: MenuItemImage(
              assetKey: builtInImageKey,
              imageBytes: userImageBytes,
              fallbackIcon: isUserImage
                  ? Icons.broken_image_outlined
                  : option?.icon ?? Icons.restaurant_outlined,
              borderRadius: const BorderRadius.horizontal(
                left: Radius.circular(15),
              ),
              cacheWidth: 256,
              cacheHeight: 192,
            ),
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    isCatalogImage
                        ? 'Gotigin catalogue image'
                        : isUserImage
                        ? 'Restaurant-owned image'
                        : 'App photo',
                    style: Theme.of(context).textTheme.labelLarge?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  const SizedBox(height: 3),
                  Text(
                    isCatalogImage
                        ? catalogImageName!
                        : isUserImage
                        ? userImageName ?? 'Selected image'
                        : option?.label ?? 'Selected menu photo',
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
          ),
          const Padding(
            padding: EdgeInsets.only(right: 12),
            child: Icon(Icons.check_circle_outline),
          ),
        ],
      ),
    );
  }
}

class _BuiltInImageChooser extends StatelessWidget {
  const _BuiltInImageChooser();

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return SafeArea(
      child: FractionallySizedBox(
        heightFactor: 0.72,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Choose an app photo',
                style: Theme.of(
                  context,
                ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
              ),
              const SizedBox(height: 6),
              Text(
                'Curated, compact menu photos for common dishes and drinks.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: 16),
              Expanded(
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    final columns = constraints.maxWidth >= 680
                        ? 4
                        : constraints.maxWidth >= 440
                        ? 3
                        : 2;
                    return GridView.builder(
                      gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                        crossAxisCount: columns,
                        mainAxisExtent: 126,
                        crossAxisSpacing: 12,
                        mainAxisSpacing: 12,
                      ),
                      itemCount: _builtInMenuImageOptions.length,
                      itemBuilder: (context, index) {
                        final option = _builtInMenuImageOptions[index];
                        return Semantics(
                          button: true,
                          label: 'Use ${option.label} app photo',
                          child: InkWell(
                            onTap: () => Navigator.of(context).pop(option.key),
                            borderRadius: const BorderRadius.all(
                              Radius.circular(16),
                            ),
                            child: Ink(
                              decoration: BoxDecoration(
                                color: colorScheme.surfaceContainerHighest,
                                borderRadius: const BorderRadius.all(
                                  Radius.circular(16),
                                ),
                                border: Border.all(
                                  color: colorScheme.outlineVariant,
                                ),
                              ),
                              child: Padding(
                                padding: const EdgeInsets.all(12),
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    SizedBox(
                                      height: 64,
                                      width: double.infinity,
                                      child: MenuItemImage(
                                        assetKey: option.key,
                                        fallbackIcon: option.icon,
                                        borderRadius: const BorderRadius.all(
                                          Radius.circular(8),
                                        ),
                                        cacheWidth: 192,
                                        cacheHeight: 128,
                                      ),
                                    ),
                                    const Spacer(),
                                    Text(
                                      option.label,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: Theme.of(context)
                                          .textTheme
                                          .labelLarge
                                          ?.copyWith(
                                            fontWeight: FontWeight.w700,
                                          ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        );
                      },
                    );
                  },
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _RemoteMenuImageSelection {
  const _RemoteMenuImageSelection({required this.image, required this.bytes});

  final RemoteMenuImage image;
  final Uint8List bytes;

  String get displayName => image.displayName;
}

class _RemoteMenuImageChooser extends StatefulWidget {
  const _RemoteMenuImageChooser();

  @override
  State<_RemoteMenuImageChooser> createState() =>
      _RemoteMenuImageChooserState();
}

class _RemoteMenuImageChooserState extends State<_RemoteMenuImageChooser> {
  final _queryController = TextEditingController();
  final _catalogue = RemoteMenuImageCatalogClient();
  RemoteMenuImagePage? _page;
  String? _error;
  var _isLoading = true;
  String? _selectingImageId;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_search);
  }

  @override
  void dispose() {
    _queryController.dispose();
    _catalogue.close();
    super.dispose();
  }

  Future<void> _search() async {
    setState(() {
      _isLoading = true;
      _error = null;
    });
    try {
      final page = await _catalogue.search(query: _queryController.text);
      if (mounted) {
        setState(() {
          _page = page;
          _isLoading = false;
        });
      }
    } on RemoteMenuImageCatalogException catch (error) {
      if (mounted) {
        setState(() {
          _error = error.message;
          _isLoading = false;
        });
      }
    }
  }

  Future<void> _select(RemoteMenuImage image) async {
    setState(() => _selectingImageId = image.imageId);
    try {
      final bytes = await _catalogue.downloadVerifiedImage(image);
      if (mounted) {
        Navigator.of(
          context,
        ).pop(_RemoteMenuImageSelection(image: image, bytes: bytes));
      }
    } on RemoteMenuImageCatalogException catch (error) {
      if (mounted) {
        setState(() {
          _selectingImageId = null;
          _error = error.message;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return SafeArea(
      child: FractionallySizedBox(
        heightFactor: 0.78,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Search Gotigin photos',
                style: Theme.of(
                  context,
                ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
              ),
              const SizedBox(height: 6),
              Text(
                'Optional online catalogue • every selected image is checked before it is saved locally.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: 16),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _queryController,
                      textInputAction: TextInputAction.search,
                      onSubmitted: (_) => _search(),
                      decoration: const InputDecoration(
                        labelText: 'Dish or cuisine',
                        prefixIcon: Icon(Icons.search),
                      ),
                    ),
                  ),
                  const SizedBox(width: 10),
                  FilledButton(
                    onPressed: _isLoading ? null : _search,
                    child: const Text('Search'),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Expanded(child: _buildResults(context)),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildResults(BuildContext context) {
    if (_isLoading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.cloud_off_outlined,
                size: 36,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
              const SizedBox(height: 12),
              Text(_error!, textAlign: TextAlign.center),
              const SizedBox(height: 12),
              Text(
                'Your app photos and restaurant-owned images remain available offline.',
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
      );
    }
    final items = _page?.items ?? const <RemoteMenuImage>[];
    if (items.isEmpty) {
      return const Center(child: Text('No matching catalogue images found.'));
    }
    return ListView.separated(
      itemCount: items.length,
      separatorBuilder: (_, _) => const Divider(height: 1),
      itemBuilder: (context, index) {
        final image = items[index];
        final isSelecting = _selectingImageId == image.imageId;
        return ListTile(
          leading: const CircleAvatar(child: Icon(Icons.restaurant_outlined)),
          title: Text(image.displayName),
          subtitle: const Text('Verified Gotigin catalogue image'),
          trailing: isSelecting
              ? const SizedBox(
                  height: 20,
                  width: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.download_outlined),
          enabled: _selectingImageId == null,
          onTap: _selectingImageId == null ? () => _select(image) : null,
        );
      },
    );
  }
}

class _BuiltInMenuImageOption {
  const _BuiltInMenuImageOption({
    required this.key,
    required this.label,
    required this.icon,
  });

  final String key;
  final String label;
  final IconData icon;
}

const _builtInMenuImageOptions = <_BuiltInMenuImageOption>[
  _BuiltInMenuImageOption(
    key: 'biryani',
    label: 'Biryani',
    icon: Icons.rice_bowl_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'curry',
    label: 'Curry',
    icon: Icons.ramen_dining_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'dosa',
    label: 'Dosa',
    icon: Icons.breakfast_dining_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'idli',
    label: 'Idli',
    icon: Icons.breakfast_dining_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'snacks',
    label: 'Snacks',
    icon: Icons.tapas_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'pizza',
    label: 'Pizza',
    icon: Icons.local_pizza_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'burger',
    label: 'Burger',
    icon: Icons.lunch_dining_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'pasta',
    label: 'Pasta',
    icon: Icons.dinner_dining_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'noodles',
    label: 'Noodles',
    icon: Icons.ramen_dining_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'rice',
    label: 'Rice',
    icon: Icons.rice_bowl_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'sandwich',
    label: 'Sandwich',
    icon: Icons.bakery_dining_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'salad',
    label: 'Salad',
    icon: Icons.eco_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'soup',
    label: 'Soup',
    icon: Icons.soup_kitchen_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'coffee',
    label: 'Coffee',
    icon: Icons.coffee_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'chai',
    label: 'Chai',
    icon: Icons.emoji_food_beverage_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'juice',
    label: 'Juice',
    icon: Icons.local_drink_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'mocktail',
    label: 'Mocktail',
    icon: Icons.wine_bar_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'dessert',
    label: 'Dessert',
    icon: Icons.cake_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'ice_cream',
    label: 'Ice cream',
    icon: Icons.icecream_outlined,
  ),
  _BuiltInMenuImageOption(
    key: 'bakery',
    label: 'Bakery',
    icon: Icons.bakery_dining_outlined,
  ),
];

_BuiltInMenuImageOption? _builtInImageForKey(String? key) {
  if (key == null) {
    return null;
  }
  for (final option in _builtInMenuImageOptions) {
    if (option.key == key) {
      return option;
    }
  }
  return null;
}

class _ProductSetupHint extends StatelessWidget {
  const _ProductSetupHint();

  @override
  Widget build(BuildContext context) {
    return Card(
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      child: const Padding(
        padding: EdgeInsets.all(20),
        child: Row(
          children: [
            Icon(Icons.category_outlined),
            SizedBox(width: 12),
            Expanded(
              child: Text(
                'Add your first category above, then create menu items with clear, exact prices.',
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ProductList extends StatelessWidget {
  const _ProductList({
    required this.products,
    required this.categories,
    required this.isSaving,
    required this.onUpdateProductPrice,
    required this.onSetProductAvailability,
    required this.onSetProductTaxTreatment,
    required this.onArchiveProduct,
    required this.onReplaceProductImage,
    required this.onDeleteUnusedProduct,
    required this.onCreateProductModifierOption,
    required this.onArchiveProductModifierOption,
  });

  final List<CommunityProductView> products;
  final List<CommunityCategoryView> categories;
  final bool isSaving;
  final _UpdateProductPrice onUpdateProductPrice;
  final _SetProductAvailability onSetProductAvailability;
  final _SetProductTaxTreatment onSetProductTaxTreatment;
  final _ArchiveProduct onArchiveProduct;
  final _ReplaceProductImage onReplaceProductImage;
  final _DeleteUnusedProduct onDeleteUnusedProduct;
  final _CreateProductModifierOption onCreateProductModifierOption;
  final _ArchiveProductModifierOption onArchiveProductModifierOption;

  @override
  Widget build(BuildContext context) {
    if (products.isEmpty) {
      return const _EmptyProductsCard();
    }

    final categoryNames = {
      for (final category in categories)
        category.categoryId: category.displayName,
    };
    return Column(
      children: [
        for (final product in products)
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: Card(
              child: ListTile(
                key: ValueKey(product.productId),
                leading: SizedBox(
                  height: 48,
                  width: 48,
                  child: MenuItemImage(
                    assetKey: product.imageAssetKey,
                    imageBytes: product.imageBytes,
                    fallbackIcon: Icons.local_cafe_outlined,
                    borderRadius: const BorderRadius.all(Radius.circular(12)),
                    cacheWidth: 96,
                    cacheHeight: 96,
                  ),
                ),
                title: Text(
                  product.displayName,
                  style: const TextStyle(fontWeight: FontWeight.w800),
                ),
                subtitle: Text(
                  '${categoryNames[product.categoryId] ?? 'Uncategorised'} • ${product.isAvailable ? 'selling now' : 'sold out'} • ${_taxTreatmentLabel(product.taxTreatment)} • saved locally',
                ),
                trailing: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      formatMinorPrice(
                        product.unitPriceMinor,
                        product.currencyCode,
                      ),
                      style: Theme.of(context).textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                    const SizedBox(width: 4),
                    PopupMenuButton<_ProductMenuAction>(
                      tooltip: 'Manage ${product.displayName}',
                      enabled: !isSaving,
                      onSelected: (action) async {
                        switch (action) {
                          case _ProductMenuAction.updatePrice:
                            await _editPrice(context, product);
                          case _ProductMenuAction.replaceImage:
                            await _replaceImage(context, product);
                          case _ProductMenuAction.setTaxTreatment:
                            await _editTaxTreatment(context, product);
                          case _ProductMenuAction.toggleAvailability:
                            await _confirmAvailability(context, product);
                          case _ProductMenuAction.archive:
                            await _confirmArchive(context, product);
                          case _ProductMenuAction.deleteUnused:
                            await _confirmDeletion(context, product);
                          case _ProductMenuAction.manageModifiers:
                            await _manageModifiers(context, product);
                        }
                      },
                      itemBuilder: (context) => [
                        PopupMenuItem(
                          value: _ProductMenuAction.manageModifiers,
                          child: _ProductMenuActionLabel(
                            icon: Icons.tune_outlined,
                            label:
                                'Modifiers (${product.modifierOptions.where((option) => !option.archived).length})',
                          ),
                        ),
                        PopupMenuItem(
                          value: _ProductMenuAction.updatePrice,
                          child: const _ProductMenuActionLabel(
                            icon: Icons.currency_rupee_outlined,
                            label: 'Update price',
                          ),
                        ),
                        PopupMenuItem(
                          value: _ProductMenuAction.replaceImage,
                          child: const _ProductMenuActionLabel(
                            icon: Icons.image_outlined,
                            label: 'Replace image',
                          ),
                        ),
                        PopupMenuItem(
                          value: _ProductMenuAction.setTaxTreatment,
                          child: const _ProductMenuActionLabel(
                            icon: Icons.percent_outlined,
                            label: 'Tax treatment',
                          ),
                        ),
                        PopupMenuItem(
                          value: _ProductMenuAction.toggleAvailability,
                          child: _ProductMenuActionLabel(
                            icon: product.isAvailable
                                ? Icons.remove_shopping_cart_outlined
                                : Icons.play_circle_outline,
                            label: product.isAvailable
                                ? 'Mark sold out'
                                : 'Resume selling',
                          ),
                        ),
                        PopupMenuItem(
                          value: _ProductMenuAction.archive,
                          child: const _ProductMenuActionLabel(
                            icon: Icons.archive_outlined,
                            label: 'Remove from active menu',
                          ),
                        ),
                        const PopupMenuDivider(),
                        const PopupMenuItem(
                          value: _ProductMenuAction.deleteUnused,
                          child: _ProductMenuActionLabel(
                            icon: Icons.delete_outline,
                            label: 'Delete unused item',
                            isDestructive: true,
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }

  Future<void> _replaceImage(
    BuildContext context,
    CommunityProductView product,
  ) async {
    final result = await FilePicker.pickFiles(
      allowMultiple: false,
      type: FileType.custom,
      allowedExtensions: const ['jpg', 'jpeg', 'png', 'webp'],
      withData: true,
    );
    final file = result?.files.singleOrNull;
    final bytes = file?.bytes;
    if (bytes == null || bytes.isEmpty) {
      return;
    }
    final prepared = await prepareCommunityMenuImage(imageBytes: bytes);
    if (prepared.isEmpty) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Image needs attention • choose a valid JPEG, PNG, or WebP under the accepted size.',
            ),
          ),
        );
      }
      return;
    }
    await onReplaceProductImage(
      productId: product.productId,
      restaurantImageBytes: prepared,
    );
  }

  Future<void> _editTaxTreatment(
    BuildContext context,
    CommunityProductView product,
  ) async {
    final selected = await showDialog<String>(
      context: context,
      builder: (dialogContext) => SimpleDialog(
        title: Text('Tax treatment — ${product.displayName}'),
        children: [
          for (final treatment in const [
            ('no_tax', 'No tax'),
            ('exclusive', 'Exclusive tax'),
            ('inclusive', 'Inclusive tax'),
          ])
            SimpleDialogOption(
              onPressed: () => Navigator.of(dialogContext).pop(treatment.$1),
              child: ListTile(
                contentPadding: EdgeInsets.zero,
                title: Text(treatment.$2),
                subtitle: Text(
                  treatment.$1 == product.taxTreatment
                      ? 'Currently selected'
                      : 'Provider-neutral local pricing only',
                ),
                trailing: treatment.$1 == product.taxTreatment
                    ? const Icon(Icons.check)
                    : null,
              ),
            ),
        ],
      ),
    );
    if (selected == null || selected == product.taxTreatment) {
      return;
    }
    await onSetProductTaxTreatment(
      productId: product.productId,
      expectedRevision: product.revision,
      taxTreatment: selected,
    );
  }

  Future<void> _manageModifiers(
    BuildContext context,
    CommunityProductView product,
  ) async {
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (_) => _ModifierOptionsSheet(
        product: product,
        onCreate: onCreateProductModifierOption,
        onArchive: onArchiveProductModifierOption,
      ),
    );
  }

  Future<void> _confirmArchive(
    BuildContext context,
    CommunityProductView product,
  ) async {
    final reason = await showDialog<String>(
      context: context,
      builder: (_) => _ProductReasonDialog(
        product: product,
        title: 'Remove from active menu?',
        detail:
            'This item will be archived, not deleted. Existing invoices, image versions, and audit history stay intact.',
        reasonLabel: 'Removal reason',
        confirmLabel: 'Archive item',
        icon: Icons.archive_outlined,
      ),
    );

    if (reason == null) {
      return;
    }
    await onArchiveProduct(
      productId: product.productId,
      expectedRevision: product.revision,
      reason: reason,
    );
  }

  Future<void> _confirmAvailability(
    BuildContext context,
    CommunityProductView product,
  ) async {
    final isResuming = !product.isAvailable;
    final reason = await showDialog<String>(
      context: context,
      builder: (_) => _ProductReasonDialog(
        product: product,
        title: isResuming ? 'Resume selling this item?' : 'Mark item sold out?',
        detail: isResuming
            ? 'The item will return to checkout. Its previous sold-out state and history stay retained.'
            : 'The item will disappear from checkout until an owner or manager resumes it. Existing orders and history stay retained.',
        reasonLabel: isResuming ? 'Resumption reason' : 'Sold-out reason',
        confirmLabel: isResuming ? 'Resume selling' : 'Mark sold out',
        icon: isResuming
            ? Icons.play_circle_outline
            : Icons.remove_shopping_cart_outlined,
      ),
    );
    if (reason == null) {
      return;
    }
    await onSetProductAvailability(
      productId: product.productId,
      expectedRevision: product.revision,
      isAvailable: isResuming,
      reason: reason,
    );
  }

  Future<void> _editPrice(
    BuildContext context,
    CommunityProductView product,
  ) async {
    final update = await showDialog<_PriceUpdateRequest>(
      context: context,
      builder: (_) => _UpdatePriceDialog(product: product),
    );
    if (update == null) {
      return;
    }
    await onUpdateProductPrice(
      productId: product.productId,
      expectedRevision: product.revision,
      unitPriceMinor: update.unitPriceMinor,
      reason: update.reason,
    );
  }

  Future<void> _confirmDeletion(
    BuildContext context,
    CommunityProductView product,
  ) async {
    final reason = await showDialog<String>(
      context: context,
      builder: (_) => _ProductReasonDialog(
        product: product,
        title: 'Delete unused item?',
        detail:
            'This is allowed only when the item has no sales, image versions, or synchronization history. Its deletion audit record remains.',
        reasonLabel: 'Deletion reason',
        confirmLabel: 'Delete unused item',
        icon: Icons.delete_outline,
        isDestructive: true,
      ),
    );
    if (reason == null) {
      return;
    }
    await onDeleteUnusedProduct(
      productId: product.productId,
      expectedRevision: product.revision,
      reason: reason,
    );
  }
}

class _ModifierOptionsSheet extends StatefulWidget {
  const _ModifierOptionsSheet({
    required this.product,
    required this.onCreate,
    required this.onArchive,
  });

  final CommunityProductView product;
  final _CreateProductModifierOption onCreate;
  final _ArchiveProductModifierOption onArchive;

  @override
  State<_ModifierOptionsSheet> createState() => _ModifierOptionsSheetState();
}

class _ModifierOptionsSheetState extends State<_ModifierOptionsSheet> {
  final _formKey = GlobalKey<FormState>();
  final _nameController = TextEditingController();
  final _priceController = TextEditingController(text: '0.00');
  var _isSubmitting = false;

  @override
  void dispose() {
    _nameController.dispose();
    _priceController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final activeOptions = widget.product.modifierOptions
        .where((option) => !option.archived)
        .toList(growable: false);
    final archivedOptions = widget.product.modifierOptions
        .where((option) => option.archived)
        .toList(growable: false);
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.82,
        child: ListView(
          padding: const EdgeInsets.fromLTRB(24, 8, 24, 28),
          children: [
            Text(
              'Modifiers · ${widget.product.displayName}',
              style: Theme.of(
                context,
              ).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 8),
            Text(
              'An option name and price adjustment cannot be edited. Archive it and add a replacement so past orders stay exact.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 18),
            Card(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Form(
                  key: _formKey,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Add optional modifier',
                        style: Theme.of(context).textTheme.titleMedium
                            ?.copyWith(fontWeight: FontWeight.w800),
                      ),
                      const SizedBox(height: 12),
                      TextFormField(
                        controller: _nameController,
                        enabled: !_isSubmitting,
                        maxLength: 120,
                        textCapitalization: TextCapitalization.words,
                        decoration: const InputDecoration(
                          labelText: 'Modifier name',
                          hintText: 'e.g. Extra cheese',
                          prefixIcon: Icon(Icons.tune_outlined),
                        ),
                        validator: (value) =>
                            value == null || value.trim().isEmpty
                            ? 'Enter a modifier name'
                            : null,
                      ),
                      const SizedBox(height: 6),
                      TextFormField(
                        controller: _priceController,
                        enabled: !_isSubmitting,
                        keyboardType: const TextInputType.numberWithOptions(
                          decimal: true,
                        ),
                        inputFormatters: [
                          FilteringTextInputFormatter.allow(RegExp(r'[0-9.,]')),
                        ],
                        decoration: const InputDecoration(
                          labelText: 'Additional price',
                          hintText: '0.00 for included',
                          prefixIcon: Icon(Icons.currency_rupee_outlined),
                        ),
                        validator: (value) =>
                            parseInrPriceToMinorUnits(value ?? '') == null
                            ? 'Enter a non-negative amount with up to 2 decimals'
                            : null,
                      ),
                      const SizedBox(height: 14),
                      Align(
                        alignment: Alignment.centerRight,
                        child: FilledButton.icon(
                          onPressed: _isSubmitting ? null : _createModifier,
                          icon: _isSubmitting
                              ? const SizedBox(
                                  height: 18,
                                  width: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.add),
                          label: const Text('Add modifier'),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
            const SizedBox(height: 18),
            Text(
              'Available at POS',
              style: Theme.of(
                context,
              ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 8),
            if (activeOptions.isEmpty)
              const _ModifierOptionsEmptyState(
                text: 'No modifiers are currently available for this item.',
              )
            else
              for (final option in activeOptions)
                Card(
                  child: ListTile(
                    title: Text(option.displayName),
                    subtitle: Text(
                      option.priceDeltaMinor == 0
                          ? 'Included'
                          : '+${formatMinorPrice(option.priceDeltaMinor, option.currencyCode)}',
                    ),
                    trailing: IconButton(
                      tooltip: 'Archive ${option.displayName}',
                      icon: const Icon(Icons.archive_outlined),
                      onPressed: _isSubmitting
                          ? null
                          : () => _confirmArchive(option),
                    ),
                  ),
                ),
            if (archivedOptions.isNotEmpty) ...[
              const SizedBox(height: 18),
              Text(
                'Archived history',
                style: Theme.of(
                  context,
                ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w800),
              ),
              const SizedBox(height: 8),
              for (final option in archivedOptions)
                ListTile(
                  leading: const Icon(Icons.history_outlined),
                  title: Text(option.displayName),
                  subtitle: Text(
                    option.priceDeltaMinor == 0
                        ? 'Included · retained for historical orders'
                        : '+${formatMinorPrice(option.priceDeltaMinor, option.currencyCode)} · retained for historical orders',
                  ),
                ),
            ],
          ],
        ),
      ),
    );
  }

  Future<void> _createModifier() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;
    final priceDeltaMinor = parseInrPriceToMinorUnits(_priceController.text);
    if (priceDeltaMinor == null) return;
    setState(() => _isSubmitting = true);
    try {
      await widget.onCreate(
        productId: widget.product.productId,
        displayName: _nameController.text,
        priceDeltaMinor: priceDeltaMinor,
      );
      if (mounted) Navigator.of(context).pop();
    } finally {
      if (mounted) setState(() => _isSubmitting = false);
    }
  }

  Future<void> _confirmArchive(CommunityModifierOptionView option) async {
    final reasonController = TextEditingController();
    try {
      final reason = await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: Text('Archive ${option.displayName}?'),
          content: TextField(
            controller: reasonController,
            autofocus: true,
            maxLength: 500,
            decoration: const InputDecoration(
              labelText: 'Archive reason',
              helperText: 'Existing orders and receipts keep this option.',
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Keep modifier'),
            ),
            FilledButton.tonal(
              onPressed: () {
                final reason = reasonController.text.trim();
                if (reason.length >= 3) {
                  Navigator.of(dialogContext).pop(reason);
                }
              },
              child: const Text('Archive'),
            ),
          ],
        ),
      );
      if (!mounted || reason == null) return;
      setState(() => _isSubmitting = true);
      try {
        await widget.onArchive(
          modifierOptionId: option.modifierOptionId,
          expectedRevision: option.revision,
          reason: reason,
        );
        if (mounted) Navigator.of(context).pop();
      } finally {
        if (mounted) setState(() => _isSubmitting = false);
      }
    } finally {
      reasonController.dispose();
    }
  }
}

class _ModifierOptionsEmptyState extends StatelessWidget {
  const _ModifierOptionsEmptyState({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Card(
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      child: Padding(padding: const EdgeInsets.all(16), child: Text(text)),
    );
  }
}

enum _ProductMenuAction {
  manageModifiers,
  updatePrice,
  replaceImage,
  setTaxTreatment,
  toggleAvailability,
  archive,
  deleteUnused,
}

class _ProductMenuActionLabel extends StatelessWidget {
  const _ProductMenuActionLabel({
    required this.icon,
    required this.label,
    this.isDestructive = false,
  });

  final IconData icon;
  final String label;
  final bool isDestructive;

  @override
  Widget build(BuildContext context) {
    final color = isDestructive ? Theme.of(context).colorScheme.error : null;
    return Row(
      children: [
        Icon(icon, size: 20, color: color),
        const SizedBox(width: 12),
        Flexible(
          child: Text(
            label,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: color == null ? null : TextStyle(color: color),
          ),
        ),
      ],
    );
  }
}

class _ProductReasonDialog extends StatefulWidget {
  const _ProductReasonDialog({
    required this.product,
    required this.title,
    required this.detail,
    required this.reasonLabel,
    required this.confirmLabel,
    required this.icon,
    this.isDestructive = false,
  });

  final CommunityProductView product;
  final String title;
  final String detail;
  final String reasonLabel;
  final String confirmLabel;
  final IconData icon;
  final bool isDestructive;

  @override
  State<_ProductReasonDialog> createState() => _ProductReasonDialogState();
}

class _ProductReasonDialogState extends State<_ProductReasonDialog> {
  final _reasonController = TextEditingController();

  @override
  void dispose() {
    _reasonController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final canArchive = _reasonController.text.trim().isNotEmpty;
    final actionColor = widget.isDestructive
        ? Theme.of(context).colorScheme.error
        : Theme.of(context).colorScheme.primary;
    return AlertDialog(
      icon: Icon(widget.icon, color: actionColor),
      title: Text(widget.title),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 440),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('${widget.product.displayName}: ${widget.detail}'),
            const SizedBox(height: 16),
            TextField(
              key: const ValueKey('archive-product-reason'),
              controller: _reasonController,
              autofocus: true,
              maxLength: 500,
              minLines: 2,
              maxLines: 4,
              textCapitalization: TextCapitalization.sentences,
              onChanged: (_) => setState(() {}),
              decoration: InputDecoration(
                labelText: widget.reasonLabel,
                hintText: 'e.g. No longer offered',
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton.icon(
          key: const ValueKey('archive-product-confirm'),
          onPressed: canArchive
              ? () => Navigator.of(context).pop(_reasonController.text.trim())
              : null,
          style: widget.isDestructive
              ? FilledButton.styleFrom(backgroundColor: actionColor)
              : null,
          icon: Icon(widget.icon),
          label: Text(widget.confirmLabel),
        ),
      ],
    );
  }
}

class _PriceUpdateRequest {
  const _PriceUpdateRequest({
    required this.unitPriceMinor,
    required this.reason,
  });

  final int unitPriceMinor;
  final String reason;
}

class _UpdatePriceDialog extends StatefulWidget {
  const _UpdatePriceDialog({required this.product});

  final CommunityProductView product;

  @override
  State<_UpdatePriceDialog> createState() => _UpdatePriceDialogState();
}

class _UpdatePriceDialogState extends State<_UpdatePriceDialog> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _priceController;
  final _reasonController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _priceController = TextEditingController(
      text: _minorUnitsToInrInput(widget.product.unitPriceMinor),
    );
  }

  @override
  void dispose() {
    _priceController.dispose();
    _reasonController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      icon: Icon(
        Icons.currency_rupee_outlined,
        color: Theme.of(context).colorScheme.primary,
      ),
      title: const Text('Update selling price'),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 440),
        child: Form(
          key: _formKey,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                '${widget.product.displayName}: this changes future sales only. Existing invoices keep their original line price.',
              ),
              const SizedBox(height: 16),
              TextFormField(
                key: const ValueKey('update-product-price'),
                controller: _priceController,
                autofocus: true,
                keyboardType: const TextInputType.numberWithOptions(
                  decimal: true,
                ),
                decoration: InputDecoration(
                  labelText:
                      'New selling price (${widget.product.currencyCode})',
                  prefixText: '${widget.product.currencyCode} ',
                ),
                validator: (value) {
                  final price = parseInrPriceToMinorUnits(value ?? '');
                  if (price == null) {
                    return 'Enter a valid amount with up to two decimal places';
                  }
                  if (price == widget.product.unitPriceMinor) {
                    return 'Enter a different price';
                  }
                  return null;
                },
              ),
              const SizedBox(height: 12),
              TextFormField(
                key: const ValueKey('update-product-price-reason'),
                controller: _reasonController,
                maxLength: 500,
                minLines: 2,
                maxLines: 4,
                textCapitalization: TextCapitalization.sentences,
                decoration: const InputDecoration(
                  labelText: 'Reason for price change',
                  hintText: 'e.g. Supplier cost increased',
                ),
                validator: (value) => value == null || value.trim().isEmpty
                    ? 'Enter a reason for this price change'
                    : null,
              ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton.icon(
          key: const ValueKey('update-product-price-confirm'),
          onPressed: () {
            if (!(_formKey.currentState?.validate() ?? false)) {
              return;
            }
            Navigator.of(context).pop(
              _PriceUpdateRequest(
                unitPriceMinor: parseInrPriceToMinorUnits(
                  _priceController.text,
                )!,
                reason: _reasonController.text.trim(),
              ),
            );
          },
          icon: const Icon(Icons.save_outlined),
          label: const Text('Save price'),
        ),
      ],
    );
  }
}

String _minorUnitsToInrInput(int minorUnits) {
  final whole = minorUnits ~/ 100;
  final fraction = (minorUnits % 100).toString().padLeft(2, '0');
  return '$whole.$fraction';
}

class _EmptyProductsCard extends StatelessWidget {
  const _EmptyProductsCard();

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(28),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(
              Icons.restaurant_outlined,
              color: Theme.of(context).colorScheme.primary,
              size: 30,
            ),
            const SizedBox(height: 14),
            Text(
              'Your menu is ready for its first item.',
              style: Theme.of(
                context,
              ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 6),
            Text(
              'Add a sellable item now. Later edits will be explicit, audited changes rather than silent overwrites.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _WorkspaceStatus extends StatelessWidget {
  const _WorkspaceStatus({required this.status});

  final String status;

  @override
  Widget build(BuildContext context) {
    final needsAttention = status.toLowerCase().contains('attention');
    final color = needsAttention
        ? Theme.of(context).colorScheme.error
        : Theme.of(context).colorScheme.primary;

    return DecoratedBox(
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.1),
        borderRadius: const BorderRadius.all(Radius.circular(14)),
      ),
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Row(
          children: [
            Icon(
              needsAttention
                  ? Icons.info_outline
                  : Icons.verified_user_outlined,
              color: color,
              size: 20,
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                status,
                style: TextStyle(color: color, fontWeight: FontWeight.w700),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EmptyCategoriesCard extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(28),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(
              Icons.category_outlined,
              color: Theme.of(context).colorScheme.primary,
              size: 30,
            ),
            const SizedBox(height: 14),
            Text(
              'Start with the way your kitchen thinks.',
              style: Theme.of(
                context,
              ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 6),
            Text(
              'For example: Starters, Main course, Hot drinks, or Desserts. You can add products next.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _KitchenWorkspace extends StatelessWidget {
  const _KitchenWorkspace({
    required this.workspace,
    required this.canOperateKitchen,
    required this.isSaving,
    required this.onAdvance,
    required this.onAcknowledgeCancellation,
    super.key,
  });
  final CommunityWorkspace workspace;
  final bool canOperateKitchen;
  final bool isSaving;
  final Future<void> Function(String ticketId, int revision, String state)
  onAdvance;
  final Future<void> Function(String ticketId) onAcknowledgeCancellation;

  @override
  Widget build(BuildContext context) {
    if (!canOperateKitchen) {
      return const _FeatureCanvas(
        icon: Icons.lock_outline,
        title: 'Kitchen Display is restricted',
        description:
            'Unlock as kitchen staff, a manager, or the owner to progress kitchen tickets.',
      );
    }
    final tickets = workspace.kitchenTickets;
    return ListView(
      padding: const EdgeInsets.all(28),
      children: [
        Text(
          'Kitchen Display',
          style: Theme.of(
            context,
          ).textTheme.headlineMedium?.copyWith(fontWeight: FontWeight.w800),
        ),
        const SizedBox(height: 8),
        Text(
          'Offline kitchen tickets only — no prices or payment information.',
          style: Theme.of(context).textTheme.bodyMedium,
        ),
        const SizedBox(height: 20),
        if (tickets.isEmpty)
          const Card(
            child: Padding(
              padding: EdgeInsets.all(28),
              child: Text('No active kitchen tickets.'),
            ),
          ),
        for (final ticket in tickets)
          Card(
            child: Padding(
              padding: const EdgeInsets.all(18),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          ticket.tableLabel ?? 'Takeaway',
                          style: Theme.of(context).textTheme.titleLarge
                              ?.copyWith(fontWeight: FontWeight.w800),
                        ),
                      ),
                      Chip(label: Text(ticket.state.replaceAll('_', ' '))),
                    ],
                  ),
                  if (ticket.cancellationPending) ...[
                    const SizedBox(height: 12),
                    Semantics(
                      liveRegion: true,
                      label:
                          'Cancellation requested. Stop work on this ticket.',
                      child: Container(
                        width: double.infinity,
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: Theme.of(context).colorScheme.errorContainer,
                          borderRadius: BorderRadius.circular(12),
                        ),
                        child: Row(
                          children: [
                            Icon(
                              Icons.stop_circle_outlined,
                              color: Theme.of(
                                context,
                              ).colorScheme.onErrorContainer,
                            ),
                            const SizedBox(width: 10),
                            Expanded(
                              child: Text(
                                'Cancellation requested — stop work. Acknowledge after the kitchen has seen this notice.',
                                style: Theme.of(context).textTheme.bodyMedium
                                    ?.copyWith(
                                      color: Theme.of(
                                        context,
                                      ).colorScheme.onErrorContainer,
                                      fontWeight: FontWeight.w700,
                                    ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                  if (ticket.kitchenNote != null) ...[
                    const SizedBox(height: 12),
                    Semantics(
                      label: 'Kitchen instruction: ${ticket.kitchenNote}',
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          color: Theme.of(
                            context,
                          ).colorScheme.tertiaryContainer,
                          borderRadius: BorderRadius.circular(12),
                        ),
                        child: Padding(
                          padding: const EdgeInsets.all(12),
                          child: Row(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Icon(
                                Icons.restaurant_outlined,
                                color: Theme.of(
                                  context,
                                ).colorScheme.onTertiaryContainer,
                              ),
                              const SizedBox(width: 10),
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      'Kitchen instruction',
                                      style: Theme.of(context)
                                          .textTheme
                                          .labelLarge
                                          ?.copyWith(
                                            color: Theme.of(
                                              context,
                                            ).colorScheme.onTertiaryContainer,
                                            fontWeight: FontWeight.w800,
                                          ),
                                    ),
                                    const SizedBox(height: 3),
                                    Text(
                                      ticket.kitchenNote!,
                                      style: TextStyle(
                                        color: Theme.of(
                                          context,
                                        ).colorScheme.onTertiaryContainer,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ],
                  const SizedBox(height: 8),
                  for (final line in ticket.lines)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 6),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            '${line.quantity} × ${line.displayName}',
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                          if (line.modifierNames.isNotEmpty)
                            Text(
                              line.modifierNames.join(' • '),
                              style: Theme.of(context).textTheme.bodySmall
                                  ?.copyWith(
                                    color: Theme.of(
                                      context,
                                    ).colorScheme.primary,
                                    fontWeight: FontWeight.w600,
                                  ),
                            ),
                        ],
                      ),
                    ),
                  const SizedBox(height: 14),
                  if (ticket.cancellationPending)
                    FilledButton.tonalIcon(
                      onPressed: isSaving
                          ? null
                          : () => onAcknowledgeCancellation(ticket.ticketId),
                      icon: const Icon(Icons.verified_outlined),
                      label: const Text('Acknowledge cancellation'),
                    )
                  else
                    FilledButton.icon(
                      onPressed: isSaving || ticket.state == 'completed'
                          ? null
                          : () => onAdvance(
                              ticket.ticketId,
                              ticket.revision,
                              switch (ticket.state) {
                                'new' => 'preparing',
                                'preparing' => 'ready',
                                _ => 'completed',
                              },
                            ),
                      icon: const Icon(Icons.arrow_forward),
                      label: Text(switch (ticket.state) {
                        'new' => 'Start preparing',
                        'preparing' => 'Mark ready',
                        _ => 'Complete ticket',
                      }),
                    ),
                ],
              ),
            ),
          ),
      ],
    );
  }
}

class _ExpenseLedgerSheet extends StatefulWidget {
  const _ExpenseLedgerSheet({
    required this.applicationSupportDirectory,
    this.branchTimeZone,
  });

  final String applicationSupportDirectory;
  final String? branchTimeZone;

  @override
  State<_ExpenseLedgerSheet> createState() => _ExpenseLedgerSheetState();
}

class _ExpenseLedgerSheetState extends State<_ExpenseLedgerSheet> {
  late Future<CommunityExpensesWorkspace> _expenses;

  @override
  void initState() {
    super.initState();
    _expenses = _load();
  }

  Future<CommunityExpensesWorkspace> _load() => loadCommunityExpenses(
    applicationSupportDirectory: widget.applicationSupportDirectory,
  );

  Future<void> _recordExpense() async {
    final category = TextEditingController();
    final description = TextEditingController();
    final amount = TextEditingController();
    var paymentMethod = 'cash';
    final request = await showDialog<_ExpenseRequest>(
      context: context,
      builder: (dialogContext) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const Text('Record expense'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  controller: category,
                  autofocus: true,
                  textCapitalization: TextCapitalization.words,
                  decoration: const InputDecoration(labelText: 'Category'),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: description,
                  maxLength: 500,
                  textCapitalization: TextCapitalization.sentences,
                  decoration: const InputDecoration(
                    labelText: 'Description',
                    helperText: 'This cannot be changed or deleted later.',
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: amount,
                  keyboardType: const TextInputType.numberWithOptions(
                    decimal: true,
                  ),
                  decoration: const InputDecoration(labelText: 'Amount (₹)'),
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<String>(
                  initialValue: paymentMethod,
                  decoration: const InputDecoration(
                    labelText: 'Payment method',
                  ),
                  items: const [
                    DropdownMenuItem(value: 'cash', child: Text('Cash')),
                    DropdownMenuItem(value: 'card', child: Text('Card')),
                    DropdownMenuItem(value: 'upi', child: Text('UPI')),
                  ],
                  onChanged: (value) => setDialogState(() {
                    paymentMethod = value ?? paymentMethod;
                  }),
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
              onPressed: () {
                final amountMinor = parseInrPriceToMinorUnits(amount.text);
                if (category.text.trim().isEmpty ||
                    description.text.trim().length < 3 ||
                    amountMinor == null ||
                    amountMinor <= 0) {
                  return;
                }
                Navigator.of(dialogContext).pop(
                  _ExpenseRequest(
                    category: category.text.trim(),
                    description: description.text.trim(),
                    amountMinor: amountMinor,
                    paymentMethod: paymentMethod,
                  ),
                );
              },
              child: const Text('Record expense'),
            ),
          ],
        ),
      ),
    );
    if (!mounted || request == null) return;
    final expenses = await recordCommunityExpense(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      category: request.category,
      description: request.description,
      amountMinor: request.amountMinor,
      paymentMethod: request.paymentMethod,
    );
    if (!mounted) return;
    setState(() => _expenses = Future.value(expenses));
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(expenses.storageStatus)));
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.82,
        child: FutureBuilder<CommunityExpensesWorkspace>(
          future: _expenses,
          builder: (context, snapshot) {
            if (snapshot.connectionState != ConnectionState.done) {
              return const Center(child: CircularProgressIndicator());
            }
            final expenses = snapshot.data;
            if (expenses == null || !expenses.available) {
              return _FeatureCanvas(
                icon: Icons.receipt_long_outlined,
                title: 'Expenses need attention',
                description:
                    expenses?.storageStatus ??
                    'The local expense ledger could not be loaded.',
              );
            }
            final currency = expenses.currencyCode ?? 'INR';
            return ListView(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 28),
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        'Expense ledger',
                        style: Theme.of(context).textTheme.headlineSmall
                            ?.copyWith(fontWeight: FontWeight.w800),
                      ),
                    ),
                    FilledButton.icon(
                      onPressed: _recordExpense,
                      icon: const Icon(Icons.add),
                      label: const Text('Record'),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Text(
                  'Recorded expenses remain available for reconciliation.',
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
                const SizedBox(height: 14),
                _WorkspaceStatus(status: expenses.storageStatus),
                const SizedBox(height: 14),
                _ReportMetricCard(
                  label: 'Recorded expenses',
                  value: _formatMinorCurrency(expenses.totalMinor, currency),
                  icon: Icons.receipt_long_outlined,
                  detail:
                      '${expenses.expenses.length} recent record${expenses.expenses.length == 1 ? '' : 's'}',
                ),
                const SizedBox(height: 14),
                for (final expense in expenses.expenses) ...[
                  Card(
                    child: ListTile(
                      leading: const Icon(Icons.receipt_long_outlined),
                      title: Text(expense.category),
                      subtitle: Text(
                        '${expense.description}\n${expense.paymentMethod.toUpperCase()} • ${formatBranchLocalTimestamp(expense.incurredAtUtc, widget.branchTimeZone)}',
                      ),
                      isThreeLine: true,
                      trailing: Text(
                        _formatMinorCurrency(
                          expense.amountMinor,
                          expense.currencyCode,
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 8),
                ],
              ],
            );
          },
        ),
      ),
    );
  }
}

class _ExpenseRequest {
  const _ExpenseRequest({
    required this.category,
    required this.description,
    required this.amountMinor,
    required this.paymentMethod,
  });
  final String category;
  final String description;
  final int amountMinor;
  final String paymentMethod;
}

class _StaffManagementSheet extends StatefulWidget {
  const _StaffManagementSheet({required this.applicationSupportDirectory});

  final String applicationSupportDirectory;

  @override
  State<_StaffManagementSheet> createState() => _StaffManagementSheetState();
}

class _StaffManagementSheetState extends State<_StaffManagementSheet> {
  late Future<CommunityStaffSecurity> _security;
  var _isSaving = false;

  @override
  void initState() {
    super.initState();
    _security = _load();
  }

  Future<CommunityStaffSecurity> _load() => loadCommunityStaffSecurity(
    applicationSupportDirectory: widget.applicationSupportDirectory,
  );

  String? _required(String? value) {
    if (value == null || value.trim().isEmpty) {
      return 'This field is required';
    }
    return null;
  }

  String? _pin(String? value) {
    if (!RegExp(r'^\d{6,12}$').hasMatch(value ?? '')) {
      return 'Use 6 to 12 digits';
    }
    return null;
  }

  void _apply(CommunityStaffSecurity security) {
    setState(() {
      _security = Future.value(security);
      _isSaving = false;
    });
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(security.storageStatus)));
  }

  Future<void> _createStaff() async {
    final name = TextEditingController();
    final pin = TextEditingController();
    final confirmPin = TextEditingController();
    final formKey = GlobalKey<FormState>();
    var role = 'cashier';
    final request = await showDialog<_StaffCreateRequest>(
      context: context,
      builder: (dialogContext) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const Text('Add local staff'),
          content: SingleChildScrollView(
            child: Form(
              key: formKey,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  TextFormField(
                    controller: name,
                    autofocus: true,
                    enabled: !_isSaving,
                    decoration: const InputDecoration(labelText: 'Staff name'),
                    validator: _required,
                  ),
                  const SizedBox(height: 12),
                  DropdownButtonFormField<String>(
                    initialValue: role,
                    decoration: const InputDecoration(labelText: 'Role'),
                    onChanged: _isSaving
                        ? null
                        : (value) {
                            if (value != null) {
                              setDialogState(() => role = value);
                            }
                          },
                    items: const [
                      DropdownMenuItem(
                        value: 'cashier',
                        child: Text('Cashier'),
                      ),
                      DropdownMenuItem(
                        value: 'kitchen',
                        child: Text('Kitchen'),
                      ),
                      DropdownMenuItem(
                        value: 'manager',
                        child: Text('Manager'),
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: pin,
                    enabled: !_isSaving,
                    obscureText: true,
                    keyboardType: TextInputType.number,
                    maxLength: 12,
                    decoration: const InputDecoration(
                      labelText: 'First PIN',
                      hintText: '6 to 12 digits',
                    ),
                    validator: _pin,
                  ),
                  TextFormField(
                    controller: confirmPin,
                    enabled: !_isSaving,
                    obscureText: true,
                    keyboardType: TextInputType.number,
                    maxLength: 12,
                    decoration: const InputDecoration(labelText: 'Confirm PIN'),
                    validator: (value) =>
                        value == pin.text ? null : 'PINs do not match',
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
              onPressed: () {
                if (formKey.currentState?.validate() ?? false) {
                  Navigator.of(dialogContext).pop(
                    _StaffCreateRequest(
                      displayName: name.text,
                      role: role,
                      pin: pin.text,
                    ),
                  );
                }
              },
              child: const Text('Add staff'),
            ),
          ],
        ),
      ),
    );
    if (request == null || !mounted) return;
    setState(() => _isSaving = true);
    final security = await createCommunityStaff(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      displayName: request.displayName,
      role: request.role,
      pin: request.pin,
    );
    if (mounted) _apply(security);
  }

  Future<void> _rotatePin(CommunityStaffView staff) async {
    final pin = TextEditingController();
    final confirmPin = TextEditingController();
    final formKey = GlobalKey<FormState>();
    final nextPin = await showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text('Set PIN — ${staff.displayName}'),
        content: Form(
          key: formKey,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextFormField(
                controller: pin,
                obscureText: true,
                keyboardType: TextInputType.number,
                maxLength: 12,
                decoration: const InputDecoration(
                  labelText: 'New PIN',
                  hintText: '6 to 12 digits',
                ),
                validator: _pin,
              ),
              TextFormField(
                controller: confirmPin,
                obscureText: true,
                keyboardType: TextInputType.number,
                maxLength: 12,
                decoration: const InputDecoration(labelText: 'Confirm new PIN'),
                validator: (value) =>
                    value == pin.text ? null : 'PINs do not match',
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
            onPressed: () {
              if (formKey.currentState?.validate() ?? false) {
                Navigator.of(dialogContext).pop(pin.text);
              }
            },
            child: const Text('Rotate PIN'),
          ),
        ],
      ),
    );
    if (nextPin == null || !mounted) return;
    setState(() => _isSaving = true);
    final security = await rotateCommunityStaffPin(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      staffId: staff.staffId,
      pin: nextPin,
    );
    if (mounted) _apply(security);
  }

  Future<void> _changeRole(CommunityStaffView staff) async {
    final reason = TextEditingController();
    final formKey = GlobalKey<FormState>();
    var role = staff.role;
    final request = await showDialog<_StaffRoleChangeRequest>(
      context: context,
      builder: (dialogContext) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: Text('Change role — ${staff.displayName}'),
          content: Form(
            key: formKey,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                DropdownButtonFormField<String>(
                  initialValue: role,
                  decoration: const InputDecoration(labelText: 'New role'),
                  onChanged: (value) {
                    if (value != null) {
                      setDialogState(() => role = value);
                    }
                  },
                  items: const [
                    DropdownMenuItem(value: 'cashier', child: Text('Cashier')),
                    DropdownMenuItem(value: 'kitchen', child: Text('Kitchen')),
                    DropdownMenuItem(value: 'manager', child: Text('Manager')),
                  ],
                ),
                const SizedBox(height: 12),
                TextFormField(
                  controller: reason,
                  autofocus: true,
                  maxLength: 500,
                  minLines: 2,
                  maxLines: 4,
                  textCapitalization: TextCapitalization.sentences,
                  decoration: const InputDecoration(
                    labelText: 'Reason',
                    helperText:
                        'The previous role remains in the audit history.',
                  ),
                  validator: (value) => (value ?? '').trim().length >= 3
                      ? null
                      : 'Enter at least 3 characters',
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton.icon(
              onPressed: () {
                if (role == staff.role) {
                  return;
                }
                if (formKey.currentState?.validate() ?? false) {
                  Navigator.of(dialogContext).pop(
                    _StaffRoleChangeRequest(
                      role: role,
                      reason: reason.text.trim(),
                    ),
                  );
                }
              },
              icon: const Icon(Icons.manage_accounts_outlined),
              label: const Text('Change role'),
            ),
          ],
        ),
      ),
    );
    if (request == null || !mounted) return;
    setState(() => _isSaving = true);
    final security = await changeCommunityStaffRole(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      staffId: staff.staffId,
      role: request.role,
      reason: request.reason,
    );
    if (mounted) _apply(security);
  }

  Future<void> _revoke(CommunityStaffView staff) async {
    final reason = TextEditingController();
    final revocationReason = await showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text('Revoke ${staff.displayName}?'),
        content: TextField(
          controller: reason,
          autofocus: true,
          maxLength: 500,
          decoration: const InputDecoration(
            labelText: 'Reason',
            helperText: 'The staff record and history are retained.',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton.tonal(
            onPressed: () {
              final value = reason.text.trim();
              if (value.isNotEmpty) {
                Navigator.of(dialogContext).pop(value);
              }
            },
            child: const Text('Revoke access'),
          ),
        ],
      ),
    );
    if (revocationReason == null || !mounted) return;
    setState(() => _isSaving = true);
    final security = await revokeCommunityStaff(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      staffId: staff.staffId,
      reason: revocationReason,
    );
    if (mounted) _apply(security);
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.82,
        child: FutureBuilder<CommunityStaffSecurity>(
          future: _security,
          builder: (context, snapshot) {
            if (snapshot.connectionState != ConnectionState.done) {
              return const Center(child: CircularProgressIndicator());
            }
            final security = snapshot.data;
            if (security == null || !security.available) {
              return _FeatureCanvas(
                icon: Icons.manage_accounts_outlined,
                title: 'Staff needs attention',
                description:
                    security?.storageStatus ?? 'Staff accounts could not load.',
              );
            }
            return ListView(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 28),
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        'Local staff',
                        style: Theme.of(context).textTheme.headlineSmall
                            ?.copyWith(fontWeight: FontWeight.w800),
                      ),
                    ),
                    FilledButton.icon(
                      onPressed: _isSaving ? null : _createStaff,
                      icon: const Icon(Icons.person_add_alt_1_outlined),
                      label: const Text('Add staff'),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Text(
                  'Owner-only. Records, credential versions, and revocations are retained locally.',
                  style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(height: 12),
                _WorkspaceStatus(status: security.storageStatus),
                const SizedBox(height: 14),
                for (final staff in security.staff) ...[
                  Card(
                    child: ListTile(
                      leading: Icon(
                        staff.active
                            ? Icons.badge_outlined
                            : Icons.person_off_outlined,
                      ),
                      title: Text(staff.displayName),
                      subtitle: Text(
                        '${_roleLabel(staff.role)} • ${staff.active ? 'active' : 'revoked'}',
                      ),
                      trailing: staff.role == 'owner'
                          ? IconButton(
                              tooltip: 'Rotate owner PIN',
                              onPressed: _isSaving
                                  ? null
                                  : () => _rotatePin(staff),
                              icon: const Icon(Icons.password_outlined),
                            )
                          : Wrap(
                              spacing: 2,
                              children: [
                                IconButton(
                                  tooltip: 'Change ${staff.displayName} role',
                                  onPressed: _isSaving || !staff.active
                                      ? null
                                      : () => _changeRole(staff),
                                  icon: const Icon(
                                    Icons.manage_accounts_outlined,
                                  ),
                                ),
                                IconButton(
                                  tooltip: 'Rotate ${staff.displayName} PIN',
                                  onPressed: _isSaving || !staff.active
                                      ? null
                                      : () => _rotatePin(staff),
                                  icon: const Icon(Icons.password_outlined),
                                ),
                                IconButton(
                                  tooltip: 'Revoke ${staff.displayName}',
                                  onPressed: _isSaving || !staff.active
                                      ? null
                                      : () => _revoke(staff),
                                  icon: const Icon(
                                    Icons.person_remove_outlined,
                                  ),
                                ),
                              ],
                            ),
                    ),
                  ),
                  const SizedBox(height: 8),
                ],
              ],
            );
          },
        ),
      ),
    );
  }
}

class _StaffCreateRequest {
  const _StaffCreateRequest({
    required this.displayName,
    required this.role,
    required this.pin,
  });

  final String displayName;
  final String role;
  final String pin;
}

class _StaffRoleChangeRequest {
  const _StaffRoleChangeRequest({required this.role, required this.reason});

  final String role;
  final String reason;
}

class _ReportsWorkspace extends StatefulWidget {
  const _ReportsWorkspace({
    required this.applicationSupportDirectory,
    this.activeStaffRole,
    super.key,
  });

  final String applicationSupportDirectory;
  final String? activeStaffRole;

  @override
  State<_ReportsWorkspace> createState() => _ReportsWorkspaceState();
}

class _ReportsWorkspaceState extends State<_ReportsWorkspace> {
  late Future<CommunitySalesSummary> _summary;
  String? _accountingDateUtc;
  var _isRefunding = false;
  var _isVoiding = false;
  var _isBackingUp = false;
  var _isVerifyingBackup = false;
  var _isRestoring = false;
  var _isExportingFinancialCsv = false;
  var _isClosingDay = false;

  bool get _canManageFinancials =>
      widget.activeStaffRole == 'owner' || widget.activeStaffRole == 'manager';

  bool get _canManageStaff => widget.activeStaffRole == 'owner';

  // Rust verifies this same active Owner requirement immediately before it
  // prepares bytes. Keeping the UI restrictive avoids presenting a file-save
  // action to staff who cannot complete it.
  bool get _canExportFinancialCsv => widget.activeStaffRole == 'owner';

  @override
  void initState() {
    super.initState();
    _summary = _load();
  }

  Future<CommunitySalesSummary> _load() => loadCommunitySalesSummary(
    applicationSupportDirectory: widget.applicationSupportDirectory,
    accountingDateUtc: _accountingDateUtc,
  );

  Future<void> _pickAccountingDay() async {
    final now = DateTime.timestamp();
    final initial = _accountingDateUtc == null
        ? DateTime.utc(now.year, now.month, now.day)
        : DateTime.tryParse('${_accountingDateUtc}T00:00:00Z') ??
              DateTime.utc(now.year, now.month, now.day);
    final picked = await showDatePicker(
      context: context,
      initialDate: initial,
      firstDate: DateTime.utc(2020),
      lastDate: DateTime.utc(now.year + 1, 12, 31),
      helpText: 'UTC accounting day',
    );
    if (picked == null || !mounted) {
      return;
    }
    final day =
        '${picked.year.toString().padLeft(4, '0')}-${picked.month.toString().padLeft(2, '0')}-${picked.day.toString().padLeft(2, '0')}';
    setState(() {
      _accountingDateUtc = day;
      _summary = _load();
    });
  }

  Future<void> _openDiagnostics() async {
    if (!_canManageStaff) return;
    recordDiagnosticBreadcrumb(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      eventCode: DiagnosticBreadcrumbs.actionDiagnosticsOpen,
    );
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (_) => _DiagnosticsSheet(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      ),
    );
  }

  Future<void> _openExpenseLedger(String? branchTimeZone) =>
      showModalBottomSheet<void>(
        context: context,
        isScrollControlled: true,
        showDragHandle: true,
        builder: (_) => _ExpenseLedgerSheet(
          applicationSupportDirectory: widget.applicationSupportDirectory,
          branchTimeZone: branchTimeZone,
        ),
      );

  Future<void> _openStaffManager() => showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    builder: (_) => _StaffManagementSheet(
      applicationSupportDirectory: widget.applicationSupportDirectory,
    ),
  );

  Future<void> _viewAuditHistory(String? branchTimeZone) async {
    final timeline = await loadCommunityAuditTimeline(
      applicationSupportDirectory: widget.applicationSupportDirectory,
    );
    if (!mounted) return;
    if (!timeline.available) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(timeline.storageStatus)));
      return;
    }
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (_) => _AuditTimelineSheet(
        timeline: timeline,
        branchTimeZone: branchTimeZone,
      ),
    );
  }

  Future<void> _viewSyncQueue(String? branchTimeZone) async {
    final queue = await loadCommunitySyncQueue(
      applicationSupportDirectory: widget.applicationSupportDirectory,
    );
    if (!mounted) return;
    if (!queue.available) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(queue.storageStatus)));
      return;
    }
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (_) =>
          _SyncQueueSheet(queue: queue, branchTimeZone: branchTimeZone),
    );
  }

  Future<void> _showCashDrawer() async {
    final drawer = await loadCommunityOpenCashDrawer(
      applicationSupportDirectory: widget.applicationSupportDirectory,
    );
    if (!mounted) return;
    final amount = TextEditingController();
    await showDialog<void>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Cash drawer'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              drawer == null
                  ? 'No cash drawer is currently open. Record the opening float.'
                  : 'Open with ${_formatMinorCurrency(drawer.openingCashMinor, drawer.currencyCode)}. Count the physical cash before closing.',
            ),
            const SizedBox(height: 12),
            TextField(
              controller: amount,
              keyboardType: const TextInputType.numberWithOptions(
                decimal: true,
              ),
              decoration: InputDecoration(
                labelText: drawer == null
                    ? 'Opening float (₹)'
                    : 'Counted cash (₹)',
              ),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Close'),
          ),
          FilledButton(
            onPressed: () async {
              final minor = parseInrPriceToMinorUnits(amount.text);
              if (minor == null || minor < 0) {
                return;
              }
              final result = drawer == null
                  ? await openCommunityCashDrawer(
                      applicationSupportDirectory:
                          widget.applicationSupportDirectory,
                      openingCashMinor: minor,
                    )
                  : await closeCommunityCashDrawer(
                      applicationSupportDirectory:
                          widget.applicationSupportDirectory,
                      sessionId: drawer.sessionId,
                      countedCashMinor: minor,
                    );
              if (!dialogContext.mounted) {
                return;
              }
              Navigator.of(dialogContext).pop();
              if (mounted) {
                ScaffoldMessenger.of(
                  context,
                ).showSnackBar(SnackBar(content: Text(result.storageStatus)));
              }
            },
            child: Text(drawer == null ? 'Open drawer' : 'Close drawer'),
          ),
        ],
      ),
    );
  }

  Future<void> _requestRefund(CommunityInvoiceView invoice) async {
    if (_isRefunding) return;
    final detail = await loadCommunityInvoiceDetail(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      invoiceId: invoice.invoiceId,
    );
    if (!mounted) return;
    if (!detail.available || detail.totalMinor == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            detail.storageStatus.isEmpty
                ? 'This invoice could not be loaded for refund.'
                : detail.storageStatus,
          ),
        ),
      );
      return;
    }

    final invoiceTotal = detail.totalMinor!;
    final alreadyRefunded = detail.refundedMinor ?? 0;
    final remaining = invoiceTotal - alreadyRefunded;
    if (remaining <= 0) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('This invoice is already fully refunded.'),
        ),
      );
      return;
    }

    final currency = detail.currencyCode ?? invoice.currencyCode;
    final remainingWhole = remaining ~/ 100;
    final remainingFraction = (remaining % 100).toString().padLeft(2, '0');
    final amountController = TextEditingController(
      text: '$remainingWhole.$remainingFraction',
    );
    final reasonController = TextEditingController();
    final approverPinController = TextEditingController();
    final security = await loadCommunityStaffSecurity(
      applicationSupportDirectory: widget.applicationSupportDirectory,
    );
    if (!mounted) {
      amountController.dispose();
      reasonController.dispose();
      approverPinController.dispose();
      return;
    }
    final approvers = security.staff
        .where(
          (staff) =>
              staff.active &&
              staff.pinConfigured &&
              (staff.role == 'owner' || staff.role == 'manager') &&
              staff.staffId != security.activeStaffId,
        )
        .toList();
    if (approvers.isEmpty) {
      amountController.dispose();
      reasonController.dispose();
      approverPinController.dispose();
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Refunds require a second active owner or manager for approval.',
          ),
        ),
      );
      return;
    }
    var selectedApproverId = approvers.first.staffId;
    final submission =
        await showDialog<
          ({
            int amountMinor,
            String reason,
            String approverStaffId,
            String approverPin,
          })
        >(
          context: context,
          builder: (dialogContext) => StatefulBuilder(
            builder: (dialogContext, setDialogState) => AlertDialog(
              title: Text('Refund invoice #${invoice.invoiceNumber}?'),
              content: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    'Remaining refundable: ${_formatMinorCurrency(remaining, currency)}. '
                    'The original invoice stays preserved. A second owner/manager must approve.',
                  ),
                  const SizedBox(height: 16),
                  TextField(
                    controller: amountController,
                    autofocus: true,
                    keyboardType: const TextInputType.numberWithOptions(
                      decimal: true,
                    ),
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(r'[0-9.,]')),
                    ],
                    decoration: InputDecoration(
                      labelText: 'Refund amount ($currency)',
                      helperText:
                          'Enter a partial amount or keep the full remaining total.',
                    ),
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: reasonController,
                    maxLength: 500,
                    textCapitalization: TextCapitalization.sentences,
                    decoration: const InputDecoration(
                      labelText: 'Reason',
                      helperText: 'Required for every refund, full or partial.',
                    ),
                  ),
                  const SizedBox(height: 12),
                  DropdownButtonFormField<String>(
                    initialValue: selectedApproverId,
                    decoration: const InputDecoration(labelText: 'Approver'),
                    items: [
                      for (final staff in approvers)
                        DropdownMenuItem(
                          value: staff.staffId,
                          child: Text('${staff.displayName} (${staff.role})'),
                        ),
                    ],
                    onChanged: (value) {
                      if (value == null) return;
                      setDialogState(() => selectedApproverId = value);
                    },
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: approverPinController,
                    obscureText: true,
                    keyboardType: TextInputType.number,
                    decoration: const InputDecoration(
                      labelText: 'Approver PIN',
                      helperText: 'Distinct from the requester session.',
                    ),
                  ),
                ],
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(dialogContext).pop(),
                  child: const Text('Cancel'),
                ),
                FilledButton.tonalIcon(
                  onPressed: () {
                    final amountMinor = parseDecimalPriceToMinorUnits(
                      amountController.text,
                    );
                    final reason = reasonController.text.trim();
                    final approverPin = approverPinController.text.trim();
                    if (amountMinor == null ||
                        amountMinor <= 0 ||
                        amountMinor > remaining ||
                        reason.length < 3 ||
                        approverPin.length < 6) {
                      return;
                    }
                    Navigator.of(dialogContext).pop((
                      amountMinor: amountMinor,
                      reason: reason,
                      approverStaffId: selectedApproverId,
                      approverPin: approverPin,
                    ));
                  },
                  icon: const Icon(Icons.undo_outlined),
                  label: const Text('Record refund'),
                ),
              ],
            ),
          ),
        );
    amountController.dispose();
    reasonController.dispose();
    approverPinController.dispose();
    if (!mounted || submission == null) return;
    setState(() => _isRefunding = true);
    final result = await refundCommunityInvoice(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      invoiceId: invoice.invoiceId,
      amountMinor: submission.amountMinor,
      reason: submission.reason,
      approverStaffId: submission.approverStaffId,
      approverPin: submission.approverPin,
      accountingDateUtc: _accountingDateUtc,
    );
    if (!mounted) return;
    setState(() {
      _isRefunding = false;
      _summary = Future.value(result);
    });
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(result.storageStatus)));
  }

  Future<void> _viewInvoice(
    CommunityInvoiceView invoice, {
    String? branchTimeZone,
  }) async {
    final detail = await loadCommunityInvoiceDetail(
      applicationSupportDirectory: widget.applicationSupportDirectory,
      invoiceId: invoice.invoiceId,
    );
    if (!mounted) return;
    if (!detail.available) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(detail.storageStatus)));
      return;
    }
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (_) =>
          _InvoiceReceiptSheet(detail: detail, branchTimeZone: branchTimeZone),
    );
  }

  Future<void> _createVerifiedBackup() async {
    if (_isBackingUp) return;
    setState(() => _isBackingUp = true);
    final result = await createCommunityLocalBackup(
      applicationSupportDirectory: widget.applicationSupportDirectory,
    );
    if (!mounted) return;
    setState(() => _isBackingUp = false);
    if (result.created) {
      recordDiagnosticBreadcrumb(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        eventCode: DiagnosticBreadcrumbs.actionBackupCreate,
      );
    }
    final checksum = result.sha256;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          result.created && checksum != null
              ? '${result.storageStatus} • checksum ${checksum.substring(0, 12)}…'
              : result.storageStatus,
        ),
      ),
    );
  }

  Future<void> _verifyLocalBackup() async {
    if (_isVerifyingBackup || !_canManageStaff) return;
    final fileNameController = TextEditingController();
    try {
      final fileName = await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: const Text('Verify local backup'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Text(
                'Enter a backup file name from verified-backups. Rust re-opens the encrypted snapshot and checks its integrity without changing live data.',
              ),
              const SizedBox(height: 12),
              TextField(
                controller: fileNameController,
                decoration: const InputDecoration(
                  labelText: 'Backup file name',
                  helperText: 'Example: restaurant-os-backup-….db',
                ),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                final value = fileNameController.text.trim();
                if (value.isEmpty ||
                    value.contains('/') ||
                    value.contains('..')) {
                  return;
                }
                Navigator.of(dialogContext).pop(value);
              },
              child: const Text('Verify'),
            ),
          ],
        ),
      );
      if (fileName == null || !mounted) return;
      setState(() => _isVerifyingBackup = true);
      final result = await verifyCommunityLocalBackup(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        backupFileName: fileName,
      );
      if (!mounted) return;
      setState(() => _isVerifyingBackup = false);
      if (result.sha256 != null) {
        recordDiagnosticBreadcrumb(
          applicationSupportDirectory: widget.applicationSupportDirectory,
          eventCode: DiagnosticBreadcrumbs.actionBackupVerify,
        );
      }
      final checksum = result.sha256;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            checksum != null
                ? '${result.storageStatus} • checksum ${checksum.substring(0, 12)}…'
                : result.storageStatus,
          ),
        ),
      );
    } finally {
      fileNameController.dispose();
    }
  }

  Future<void> _closeAccountingDay(CommunitySalesSummary summary) async {
    if (_isClosingDay || !_canManageFinancials || summary.dayClosed) return;
    final day = summary.accountingDateUtc;
    if (day == null || day.isEmpty) return;
    final reasonController = TextEditingController();
    try {
      final reason = await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: Text('Close UTC day $day?'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                'This freezes a snapshot of today’s report totals. Reopen is not supported. Accounting day boundaries remain UTC.',
              ),
              const SizedBox(height: 12),
              TextField(
                controller: reasonController,
                decoration: const InputDecoration(labelText: 'Close reason'),
                maxLines: 2,
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                final value = reasonController.text.trim();
                if (value.length < 3) return;
                Navigator.of(dialogContext).pop(value);
              },
              child: const Text('Close day'),
            ),
          ],
        ),
      );
      if (reason == null || !mounted) return;
      setState(() => _isClosingDay = true);
      final closed = await closeCommunityAccountingDay(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        accountingDateUtc: day,
        reason: reason,
      );
      if (!mounted) return;
      setState(() {
        _isClosingDay = false;
        _summary = Future.value(closed);
      });
      if (closed.available && closed.dayClosed) {
        recordDiagnosticBreadcrumb(
          applicationSupportDirectory: widget.applicationSupportDirectory,
          eventCode: DiagnosticBreadcrumbs.actionDayClose,
        );
      }
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(closed.storageStatus)));
    } finally {
      reasonController.dispose();
    }
  }

  Future<void> _restoreVerifiedBackup() async {
    if (_isRestoring || !_canManageStaff) return;
    final fileNameController = TextEditingController();
    try {
      final fileName = await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: const Text('Restore verified backup'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Text(
                'Same-installation restore only. Enter a backup file name from verified-backups. The live database stays unchanged; a restaurant-os.restored.db file is written beside it.',
              ),
              const SizedBox(height: 12),
              TextField(
                controller: fileNameController,
                decoration: const InputDecoration(
                  labelText: 'Backup file name',
                  helperText: 'Example: restaurant-os-backup-….db',
                ),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                final value = fileNameController.text.trim();
                if (value.isEmpty ||
                    value.contains('/') ||
                    value.contains('..')) {
                  return;
                }
                Navigator.of(dialogContext).pop(value);
              },
              child: const Text('Restore beside live'),
            ),
          ],
        ),
      );
      if (fileName == null || !mounted) return;
      setState(() => _isRestoring = true);
      final result = await restoreCommunityLocalBackup(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        backupFileName: fileName,
      );
      if (!mounted) return;
      setState(() => _isRestoring = false);
      if (result.created) {
        recordDiagnosticBreadcrumb(
          applicationSupportDirectory: widget.applicationSupportDirectory,
          eventCode: DiagnosticBreadcrumbs.actionBackupRestore,
        );
      }
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(result.storageStatus)));
    } finally {
      fileNameController.dispose();
    }
  }

  Future<void> _requestVoid(CommunityInvoiceView invoice) async {
    final reasonController = TextEditingController();
    final approverPinController = TextEditingController();
    try {
      final security = await loadCommunityStaffSecurity(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      if (!mounted) return;
      final approvers = security.staff
          .where(
            (staff) =>
                staff.active &&
                staff.pinConfigured &&
                (staff.role == 'owner' || staff.role == 'manager') &&
                staff.staffId != security.activeStaffId,
          )
          .toList();
      if (approvers.isEmpty) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Voids require a second active owner or manager for approval.',
            ),
          ),
        );
        return;
      }
      var selectedApproverId = approvers.first.staffId;
      final submission =
          await showDialog<
            ({String reason, String approverStaffId, String approverPin})
          >(
            context: context,
            builder: (dialogContext) => StatefulBuilder(
              builder: (dialogContext, setDialogState) => AlertDialog(
                title: Text('Void invoice #${invoice.invoiceNumber}?'),
                content: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Text(
                      'Records an immutable void. The original invoice stays intact. Refunded invoices cannot be voided. A second owner/manager must approve.',
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      controller: reasonController,
                      maxLength: 500,
                      autofocus: true,
                      textCapitalization: TextCapitalization.sentences,
                      decoration: const InputDecoration(labelText: 'Reason'),
                    ),
                    const SizedBox(height: 12),
                    DropdownButtonFormField<String>(
                      initialValue: selectedApproverId,
                      decoration: const InputDecoration(labelText: 'Approver'),
                      items: [
                        for (final staff in approvers)
                          DropdownMenuItem(
                            value: staff.staffId,
                            child: Text('${staff.displayName} (${staff.role})'),
                          ),
                      ],
                      onChanged: (value) {
                        if (value == null) return;
                        setDialogState(() => selectedApproverId = value);
                      },
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      controller: approverPinController,
                      obscureText: true,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Approver PIN',
                      ),
                    ),
                  ],
                ),
                actions: [
                  TextButton(
                    onPressed: () => Navigator.of(dialogContext).pop(),
                    child: const Text('Cancel'),
                  ),
                  FilledButton(
                    onPressed: () {
                      final reason = reasonController.text.trim();
                      final approverPin = approverPinController.text.trim();
                      if (reason.length < 3 || approverPin.length < 6) return;
                      Navigator.of(dialogContext).pop((
                        reason: reason,
                        approverStaffId: selectedApproverId,
                        approverPin: approverPin,
                      ));
                    },
                    child: const Text('Void invoice'),
                  ),
                ],
              ),
            ),
          );
      if (submission == null || !mounted) return;
      setState(() => _isVoiding = true);
      final result = await voidCommunityInvoice(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        invoiceId: invoice.invoiceId,
        reason: submission.reason,
        approverStaffId: submission.approverStaffId,
        approverPin: submission.approverPin,
        accountingDateUtc: _accountingDateUtc,
      );
      if (!mounted) return;
      setState(() {
        _isVoiding = false;
        _summary = Future.value(result);
      });
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(result.storageStatus)));
    } finally {
      reasonController.dispose();
      approverPinController.dispose();
    }
  }

  Future<void> _exportVerifiedFinancialCsv() async {
    if (_isExportingFinancialCsv || !_canExportFinancialCsv) {
      return;
    }
    // Browser downloads do not give the owner a trustworthy native
    // destination-selection step. Keep this security-sensitive export
    // unavailable there until a reviewed browser save-location policy exists.
    if (kIsWeb) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Financial CSV export requires a platform save dialog with an explicit destination.',
          ),
        ),
      );
      return;
    }

    setState(() => _isExportingFinancialCsv = true);
    try {
      final export = await prepareCommunityFinancialCsv(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      if (!mounted) {
        return;
      }
      if (!export.available || export.csvBytes.isEmpty) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(export.storageStatus)));
        return;
      }

      // File Picker receives the prepared bytes only after the active owner
      // initiated this action and chooses a destination in the native dialog.
      // It performs the platform-specific write; this app never chooses a
      // default file path or writes an unencrypted report silently.
      final savedDestination = await FilePicker.saveFile(
        dialogTitle: 'Save verified financial CSV',
        fileName: 'restaurant-financial-export.csv',
        type: FileType.custom,
        allowedExtensions: const ['csv'],
        bytes: export.csvBytes,
        lockParentWindow: true,
      );
      if (!mounted) {
        return;
      }
      if (savedDestination == null) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Financial CSV export cancelled • no file was saved.',
            ),
          ),
        );
        return;
      }
      recordDiagnosticBreadcrumb(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        eventCode: DiagnosticBreadcrumbs.actionExportCsv,
      );
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Financial CSV saved • ${export.recordCount} aggregate records • ${export.byteLength} bytes',
          ),
        ),
      );
    } on PlatformException {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'The save dialog could not complete. Verify the selected destination before retrying.',
            ),
          ),
        );
      }
    } catch (_) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Financial CSV export could not complete. Verify the selected destination before retrying.',
            ),
          ),
        );
      }
    } finally {
      if (mounted) {
        setState(() => _isExportingFinancialCsv = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<CommunitySalesSummary>(
      future: _summary,
      builder: (context, snapshot) {
        if (snapshot.connectionState != ConnectionState.done) {
          return const Center(child: CircularProgressIndicator());
        }
        final summary = snapshot.data;
        if (summary == null || !summary.available) {
          return _FeatureCanvas(
            icon: Icons.insights_outlined,
            title: 'Reports need attention',
            description:
                summary?.storageStatus ??
                'The encrypted local report could not be loaded.',
          );
        }
        final currency = summary.currencyCode ?? '—';
        return ListView(
          padding: const EdgeInsets.all(28),
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  flex: 3,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Local Sales Report',
                        style: Theme.of(context).textTheme.headlineMedium
                            ?.copyWith(fontWeight: FontWeight.w800),
                      ),
                      const SizedBox(height: 6),
                      Text(
                        'UTC day ${summary.accountingDateUtc ?? '—'} • ${summary.branchTimeZone ?? 'local zone'} display • offline source of truth',
                        style: Theme.of(context).textTheme.bodyMedium,
                      ),
                      if (summary.dayClosed) ...[
                        const SizedBox(height: 6),
                        Text(
                          'Day closed ${formatBranchLocalTimestamp(summary.dayClosedAtUtc ?? '', summary.branchTimeZone)}${summary.dayCloseReason == null ? '' : ' • ${summary.dayCloseReason}'}',
                          style: Theme.of(context).textTheme.bodySmall
                              ?.copyWith(
                                color: Theme.of(context).colorScheme.primary,
                                fontWeight: FontWeight.w600,
                              ),
                        ),
                      ],
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                Flexible(
                  flex: 2,
                  child: Align(
                    alignment: AlignmentDirectional.topEnd,
                    child: Wrap(
                      alignment: WrapAlignment.end,
                      spacing: 2,
                      runSpacing: 2,
                      children: [
                        IconButton(
                          tooltip: 'Choose UTC accounting day',
                          onPressed: _pickAccountingDay,
                          icon: const Icon(Icons.calendar_today_outlined),
                        ),
                        IconButton(
                          tooltip: summary.dayClosed
                              ? 'This UTC day is already closed'
                              : 'Close UTC accounting day',
                          onPressed:
                              !_canManageFinancials ||
                                  _isClosingDay ||
                                  summary.dayClosed
                              ? null
                              : () => _closeAccountingDay(summary),
                          icon: _isClosingDay
                              ? const SizedBox(
                                  height: 18,
                                  width: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : Icon(
                                  summary.dayClosed
                                      ? Icons.lock_outline
                                      : Icons.event_available_outlined,
                                ),
                        ),
                        IconButton(
                          tooltip: 'Export verified financial CSV',
                          onPressed:
                              !_canExportFinancialCsv ||
                                  _isExportingFinancialCsv
                              ? null
                              : _exportVerifiedFinancialCsv,
                          icon: _isExportingFinancialCsv
                              ? const SizedBox(
                                  height: 18,
                                  width: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.file_download_outlined),
                        ),
                        IconButton(
                          tooltip: 'Create verified local backup',
                          onPressed: !_canManageStaff || _isBackingUp
                              ? null
                              : _createVerifiedBackup,
                          icon: _isBackingUp
                              ? const SizedBox(
                                  height: 18,
                                  width: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.backup_outlined),
                        ),
                        IconButton(
                          tooltip: 'Verify local backup integrity',
                          onPressed: !_canManageStaff || _isVerifyingBackup
                              ? null
                              : _verifyLocalBackup,
                          icon: _isVerifyingBackup
                              ? const SizedBox(
                                  height: 18,
                                  width: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.verified_user_outlined),
                        ),
                        IconButton(
                          tooltip: 'Restore verified backup beside live data',
                          onPressed: !_canManageStaff || _isRestoring
                              ? null
                              : _restoreVerifiedBackup,
                          icon: _isRestoring
                              ? const SizedBox(
                                  height: 18,
                                  width: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.settings_backup_restore),
                        ),
                        IconButton(
                          tooltip: 'Manage local staff',
                          onPressed: _canManageStaff ? _openStaffManager : null,
                          icon: const Icon(Icons.manage_accounts_outlined),
                        ),
                        IconButton(
                          tooltip: 'Local diagnostics and voluntary share',
                          onPressed: _canManageStaff ? _openDiagnostics : null,
                          icon: const Icon(Icons.bug_report_outlined),
                        ),
                        IconButton(
                          tooltip: 'View verified audit history',
                          onPressed: _canManageStaff
                              ? () => _viewAuditHistory(summary.branchTimeZone)
                              : null,
                          icon: const Icon(Icons.history_outlined),
                        ),
                        IconButton(
                          tooltip: 'View local sync queue',
                          onPressed: _canManageStaff
                              ? () => _viewSyncQueue(summary.branchTimeZone)
                              : null,
                          icon: const Icon(Icons.cloud_sync_outlined),
                        ),
                        IconButton(
                          tooltip: 'Open expense ledger',
                          onPressed: _canManageFinancials
                              ? () => _openExpenseLedger(summary.branchTimeZone)
                              : null,
                          icon: const Icon(Icons.receipt_long_outlined),
                        ),
                        IconButton(
                          tooltip: 'View cash drawer',
                          onPressed: _canManageFinancials
                              ? _showCashDrawer
                              : null,
                          icon: const Icon(Icons.point_of_sale_outlined),
                        ),
                        IconButton(
                          tooltip: 'Refresh local report',
                          onPressed: () => setState(() => _summary = _load()),
                          icon: const Icon(Icons.refresh),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 22),
            _ReportMetricCard(
              label: 'Finalized sales',
              value: _formatMinorCurrency(summary.totalMinor, currency),
              icon: Icons.receipt_long_outlined,
              detail:
                  '${summary.invoiceCount} invoice${summary.invoiceCount == 1 ? '' : 's'}',
            ),
            const SizedBox(height: 14),
            Wrap(
              spacing: 14,
              runSpacing: 14,
              children: [
                _ReportMetricCard(
                  label: 'Cash',
                  value: _formatMinorCurrency(summary.cashMinor, currency),
                  icon: Icons.payments_outlined,
                  detail: 'Net of refunds',
                ),
                _ReportMetricCard(
                  label: 'Card',
                  value: _formatMinorCurrency(summary.cardMinor, currency),
                  icon: Icons.credit_card_outlined,
                  detail: 'Net of refunds',
                ),
                _ReportMetricCard(
                  label: 'UPI',
                  value: _formatMinorCurrency(summary.upiMinor, currency),
                  icon: Icons.qr_code_2_outlined,
                  detail: 'Net of refunds',
                ),
                _ReportMetricCard(
                  label: 'Refunds',
                  value: _formatMinorCurrency(summary.refundMinor, currency),
                  icon: Icons.undo_outlined,
                  detail: 'Retained correction total',
                ),
                _ReportMetricCard(
                  label: 'Discounts',
                  value: _formatMinorCurrency(summary.discountMinor, currency),
                  icon: Icons.sell_outlined,
                  detail: 'Finalized invoice discounts',
                ),
                _ReportMetricCard(
                  label: 'Tax collected',
                  value: _formatMinorCurrency(summary.taxMinor, currency),
                  icon: Icons.account_balance_outlined,
                  detail: 'Finalized invoice tax total',
                ),
                _ReportMetricCard(
                  label: 'Expenses',
                  value: _formatMinorCurrency(summary.expenseMinor, currency),
                  icon: Icons.receipt_long_outlined,
                  detail: 'Recorded operating expenses',
                ),
              ],
            ),
            const SizedBox(height: 22),
            Text(
              'Top items',
              style: Theme.of(
                context,
              ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 4),
            Text(
              'Gross finalized item sales. Invoice-level refunds remain separate.',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 10),
            Card(
              child: summary.topItems.isEmpty
                  ? const Padding(
                      padding: EdgeInsets.all(18),
                      child: Text('No finalized item sales yet.'),
                    )
                  : Column(
                      children: [
                        for (
                          var index = 0;
                          index < summary.topItems.length;
                          index++
                        ) ...[
                          if (index > 0) const Divider(height: 1),
                          ListTile(
                            leading: CircleAvatar(child: Text('${index + 1}')),
                            title: Text(summary.topItems[index].displayName),
                            subtitle: Text(
                              '${summary.topItems[index].quantity} unit${summary.topItems[index].quantity == 1 ? '' : 's'} sold',
                            ),
                            trailing: Text(
                              _formatMinorCurrency(
                                summary.topItems[index].grossTotalMinor,
                                summary.topItems[index].currencyCode,
                              ),
                              style: const TextStyle(
                                fontWeight: FontWeight.w800,
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
            ),
            const SizedBox(height: 22),
            Text(
              'Recent invoices',
              style: Theme.of(
                context,
              ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 10),
            Card(
              child: summary.recentInvoices.isEmpty
                  ? const Padding(
                      padding: EdgeInsets.all(18),
                      child: Text('No finalized invoices yet.'),
                    )
                  : Column(
                      children: [
                        for (
                          var index = 0;
                          index < summary.recentInvoices.length;
                          index++
                        ) ...[
                          if (index > 0) const Divider(height: 1),
                          ListTile(
                            onTap: () => _viewInvoice(
                              summary.recentInvoices[index],
                              branchTimeZone: summary.branchTimeZone,
                            ),
                            leading: const Icon(Icons.receipt_long_outlined),
                            title: Text(
                              'Invoice #${summary.recentInvoices[index].invoiceNumber}',
                            ),
                            subtitle: Text(
                              '${summary.recentInvoices[index].paymentMethod.toUpperCase()} • ${formatBranchLocalTimestamp(summary.recentInvoices[index].finalizedAtUtc, summary.branchTimeZone)}',
                            ),
                            trailing: Wrap(
                              crossAxisAlignment: WrapCrossAlignment.center,
                              spacing: 8,
                              children: [
                                Text(
                                  _formatMinorCurrency(
                                    summary.recentInvoices[index].totalMinor,
                                    summary.recentInvoices[index].currencyCode,
                                  ),
                                  style: const TextStyle(
                                    fontWeight: FontWeight.w800,
                                  ),
                                ),
                                IconButton(
                                  tooltip: 'View immutable receipt',
                                  onPressed: () => _viewInvoice(
                                    summary.recentInvoices[index],
                                    branchTimeZone: summary.branchTimeZone,
                                  ),
                                  icon: const Icon(Icons.visibility_outlined),
                                ),
                                IconButton(
                                  tooltip: 'Record refund',
                                  onPressed:
                                      !_canManageFinancials || _isRefunding
                                      ? null
                                      : () => _requestRefund(
                                          summary.recentInvoices[index],
                                        ),
                                  icon: const Icon(Icons.undo_outlined),
                                ),
                                IconButton(
                                  tooltip: 'Void invoice',
                                  onPressed: !_canManageFinancials || _isVoiding
                                      ? null
                                      : () => _requestVoid(
                                          summary.recentInvoices[index],
                                        ),
                                  icon: const Icon(Icons.block_outlined),
                                ),
                              ],
                            ),
                          ),
                        ],
                      ],
                    ),
            ),
            const SizedBox(height: 22),
            Card(
              child: Padding(
                padding: const EdgeInsets.all(18),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(
                      Icons.verified_user_outlined,
                      color: Theme.of(context).colorScheme.primary,
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Text(
                        'Integrity verified: SQLCipher pages, schema v${summary.schemaVersion}, foreign keys, and ${summary.auditEventCount} audit event${summary.auditEventCount == 1 ? '' : 's'} passed. These figures come only from immutable finalized invoices and payments; held orders and kitchen tickets are excluded.',
                        style: Theme.of(context).textTheme.bodyMedium,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _DiagnosticsSheet extends StatefulWidget {
  const _DiagnosticsSheet({required this.applicationSupportDirectory});

  final String applicationSupportDirectory;

  @override
  State<_DiagnosticsSheet> createState() => _DiagnosticsSheetState();
}

class _DiagnosticsSheetState extends State<_DiagnosticsSheet> {
  late Future<CommunityDiagnosticsWorkspace> _workspace;
  var _busy = false;

  @override
  void initState() {
    super.initState();
    _workspace = _load();
  }

  Future<CommunityDiagnosticsWorkspace> _load() => loadCommunityDiagnostics(
    applicationSupportDirectory: widget.applicationSupportDirectory,
  );

  Future<void> _exportPack() async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      final pack = await exportCommunityDiagnosticsPack(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      if (!mounted) return;
      if (!pack.available || pack.jsonBytes.isEmpty) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(pack.storageStatus)));
        return;
      }
      if (kIsWeb) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Diagnostics export requires a platform save dialog.',
            ),
          ),
        );
        return;
      }
      final savedPath = await FilePicker.saveFile(
        dialogTitle: 'Save redacted diagnostics pack',
        fileName: 'restaurant-os-diagnostics.json',
        type: FileType.custom,
        allowedExtensions: const ['json'],
        bytes: pack.jsonBytes,
        lockParentWindow: true,
      );
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            savedPath == null
                ? 'Diagnostics export cancelled • no file was written'
                : '${pack.storageStatus} • saved',
          ),
        ),
      );
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _sharePack() async {
    if (_busy) return;
    final purpose = await showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Share diagnostics with Gotigin?'),
        content: const SingleChildScrollView(
          child: Text(
            'Logging on this device uses only technical event codes to help keep the app reliable.\n\n'
            'Sharing is optional and happens only if you choose Share.\n\n'
            'The pack does not include PINs, customer contacts, payment card data, or database keys.\n\n'
            'Shared data is used to diagnose this issue and improve the product.\n\n'
            'Choose why you are sharing:',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () =>
                Navigator.of(dialogContext).pop('product_improvement'),
            child: const Text('Improve the product'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop('support_issue'),
            child: const Text('Help with this issue'),
          ),
        ],
      ),
    );
    if (purpose == null || !mounted) return;
    setState(() => _busy = true);
    try {
      final prepared = await prepareCommunityDiagnosticsShare(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        purpose: purpose,
      );
      if (!mounted) return;
      if (!prepared.prepared) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(prepared.storageStatus)));
        return;
      }
      final upload = await uploadDiagnosticsSharePack(
        prepared: prepared,
        purpose: purpose,
      );
      await recordCommunityDiagnosticsShareOutcome(
        applicationSupportDirectory: widget.applicationSupportDirectory,
        uploaded: upload.uploaded,
      );
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(upload.status)));
      setState(() => _workspace = _load());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _clearLocal() async {
    if (_busy) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Clear local diagnostics?'),
        content: const Text(
          'This removes allow-listed diagnostic events from this device only. Restaurant data is unchanged.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Clear'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    setState(() => _busy = true);
    try {
      final cleared = await clearCommunityDiagnostics(
        applicationSupportDirectory: widget.applicationSupportDirectory,
      );
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            cleared
                ? 'Local diagnostics cleared'
                : 'Diagnostics could not be cleared',
          ),
        ),
      );
      setState(() => _workspace = _load());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.82,
        child: FutureBuilder<CommunityDiagnosticsWorkspace>(
          future: _workspace,
          builder: (context, snapshot) {
            if (snapshot.connectionState != ConnectionState.done) {
              return const Center(child: CircularProgressIndicator());
            }
            final workspace = snapshot.data;
            if (workspace == null || !workspace.available) {
              return _FeatureCanvas(
                icon: Icons.bug_report_outlined,
                title: 'Diagnostics need attention',
                description:
                    workspace?.storageStatus ??
                    'Local diagnostics could not be loaded.',
              );
            }
            return ListView(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 28),
              children: [
                Text(
                  'Local diagnostics',
                  style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
                const SizedBox(height: 8),
                Text(
                  'Technical event codes stay on this device. Sharing with Gotigin is optional and requires your consent each time.',
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
                const SizedBox(height: 10),
                _WorkspaceStatus(status: workspace.storageStatus),
                const SizedBox(height: 8),
                Text(
                  diagnosticsShareEndpointConfigured
                      ? 'Cloud share endpoint is configured for this build.'
                      : 'Cloud share endpoint is not configured yet — you can still export a pack.',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                const SizedBox(height: 14),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    FilledButton.icon(
                      onPressed: _busy ? null : _exportPack,
                      icon: const Icon(Icons.save_alt_outlined),
                      label: const Text('Export pack'),
                    ),
                    FilledButton.tonalIcon(
                      onPressed: _busy ? null : _sharePack,
                      icon: const Icon(Icons.cloud_upload_outlined),
                      label: const Text('Share with Gotigin'),
                    ),
                    OutlinedButton.icon(
                      onPressed: _busy ? null : _clearLocal,
                      icon: const Icon(Icons.delete_outline),
                      label: const Text('Clear local'),
                    ),
                  ],
                ),
                const SizedBox(height: 18),
                Text(
                  'Recent events',
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 8),
                if (workspace.events.isEmpty)
                  const Text('No diagnostic events recorded yet.')
                else
                  for (final event in workspace.events.reversed) ...[
                    ListTile(
                      contentPadding: EdgeInsets.zero,
                      title: Text(event.eventCode),
                      subtitle: Text(
                        '${event.component} • ${event.outcome}'
                        '${event.detailCode == null ? '' : ' • ${event.detailCode}'}'
                        '${event.durationMs == null ? '' : ' • ${event.durationMs} ms'}',
                      ),
                      trailing: Text(
                        formatBranchLocalTimestamp(event.occurredAtUtc),
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ),
                    const Divider(height: 1),
                  ],
              ],
            );
          },
        ),
      ),
    );
  }
}

class _AuditTimelineSheet extends StatelessWidget {
  const _AuditTimelineSheet({required this.timeline, this.branchTimeZone});

  final CommunityAuditTimeline timeline;
  final String? branchTimeZone;

  String _displayEventType(String eventType) =>
      eventType.replaceAll('.', ' › ').replaceAll('_', ' ');

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.82,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Verified audit history',
                    style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Latest 100 branch events. This safe view excludes event payloads, device identifiers, and credentials.',
                    style: Theme.of(context).textTheme.bodyMedium,
                  ),
                  const SizedBox(height: 10),
                  _WorkspaceStatus(status: timeline.storageStatus),
                ],
              ),
            ),
            const Divider(height: 1),
            Expanded(
              child: timeline.events.isEmpty
                  ? const Center(child: Text('No local audit events yet.'))
                  : ListView.separated(
                      padding: const EdgeInsets.fromLTRB(24, 12, 24, 28),
                      itemCount: timeline.events.length,
                      separatorBuilder: (_, _) => const Divider(height: 1),
                      itemBuilder: (context, index) {
                        final event = timeline.events[index];
                        return ListTile(
                          leading: const Icon(Icons.verified_user_outlined),
                          title: Text(_displayEventType(event.eventType)),
                          subtitle: Text(
                            formatBranchLocalTimestamp(
                              event.occurredAtUtc,
                              branchTimeZone,
                            ),
                          ),
                          trailing: Text('Event ${event.sequence}'),
                        );
                      },
                    ),
            ),
          ],
        ),
      ),
    );
  }
}

class _SyncQueueSheet extends StatelessWidget {
  const _SyncQueueSheet({required this.queue, this.branchTimeZone});

  final CommunitySyncQueue queue;
  final String? branchTimeZone;

  String _displayEventType(String eventType) =>
      eventType.replaceAll('.', ' › ').replaceAll('_', ' ');

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.82,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Local sync queue',
                    style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Immutable local operations waiting for Professional acknowledgement. This view stays offline and excludes payloads, hashes, and device identifiers.',
                    style: Theme.of(context).textTheme.bodyMedium,
                  ),
                  const SizedBox(height: 10),
                  _WorkspaceStatus(status: queue.storageStatus),
                ],
              ),
            ),
            const Divider(height: 1),
            Expanded(
              child: queue.operations.isEmpty
                  ? const Center(child: Text('No pending sync operations.'))
                  : ListView.separated(
                      padding: const EdgeInsets.fromLTRB(24, 12, 24, 28),
                      itemCount: queue.operations.length,
                      separatorBuilder: (_, _) => const Divider(height: 1),
                      itemBuilder: (context, index) {
                        final operation = queue.operations[index];
                        return ListTile(
                          leading: const Icon(Icons.cloud_queue_outlined),
                          title: Text(_displayEventType(operation.eventType)),
                          subtitle: Text(
                            '${operation.entityType} • ${formatBranchLocalTimestamp(operation.createdAtUtc, branchTimeZone)}',
                          ),
                          trailing: Text('Event ${operation.sequence}'),
                        );
                      },
                    ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ReportMetricCard extends StatelessWidget {
  const _ReportMetricCard({
    required this.label,
    required this.value,
    required this.icon,
    this.detail,
  });

  final String label;
  final String value;
  final IconData icon;
  final String? detail;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 230,
      child: Card(
        child: Padding(
          padding: const EdgeInsets.all(18),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(icon, color: Theme.of(context).colorScheme.primary),
              const SizedBox(height: 14),
              Text(label, style: Theme.of(context).textTheme.labelLarge),
              const SizedBox(height: 4),
              Text(
                value,
                style: Theme.of(
                  context,
                ).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w800),
              ),
              if (detail != null) ...[
                const SizedBox(height: 4),
                Text(detail!, style: Theme.of(context).textTheme.bodySmall),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _InvoiceReceiptSheet extends StatelessWidget {
  const _InvoiceReceiptSheet({required this.detail, this.branchTimeZone});

  final CommunityInvoiceDetail detail;
  final String? branchTimeZone;

  String get _currencyCode => detail.currencyCode ?? 'INR';

  String _receiptText() {
    final lines = <String>[
      'GOTIGIN RESTAURANT OS',
      'Invoice #${detail.invoiceNumber ?? '—'}',
      detail.finalizedAtUtc == null
          ? ''
          : formatBranchLocalTimestamp(detail.finalizedAtUtc!, branchTimeZone),
      'Fulfillment: ${(detail.fulfillment ?? '—').replaceAll('_', ' ')}',
      '',
      ...detail.lines.map(
        (line) => [
          '${line.displayName} × ${line.quantity}  ${_formatMinorCurrency(line.lineTotalMinor, _currencyCode)}',
          if (line.modifierNames.isNotEmpty)
            '  ${line.modifierNames.join(' • ')}',
        ].join('\n'),
      ),
      '',
      'Subtotal  ${_formatMinorCurrency(detail.subtotalMinor ?? 0, _currencyCode)}',
      if ((detail.discountMinor ?? 0) > 0)
        'Discount  ${_formatMinorCurrency(detail.discountMinor!, _currencyCode)}',
      if ((detail.taxMinor ?? 0) > 0)
        'Tax       ${_formatMinorCurrency(detail.taxMinor!, _currencyCode)}',
      'Total     ${_formatMinorCurrency(detail.totalMinor ?? 0, _currencyCode)}',
      ...detail.payments.map(
        (payment) =>
            '${payment.paymentMethod.toUpperCase()}  ${_formatMinorCurrency(payment.amountMinor, _currencyCode)}',
      ),
      if ((detail.refundedMinor ?? 0) > 0)
        'Refunded  ${_formatMinorCurrency(detail.refundedMinor!, _currencyCode)}',
      '',
      'Loaded from immutable local records.',
    ];
    return lines.where((line) => line.isNotEmpty).join('\n');
  }

  Future<void> _copy(BuildContext context) async {
    await Clipboard.setData(ClipboardData(text: _receiptText()));
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Receipt copied to clipboard.')),
      );
    }
  }

  Future<void> _save(BuildContext context) async {
    if (kIsWeb) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Receipt export requires a platform save dialog with an explicit destination.',
            ),
          ),
        );
      }
      return;
    }

    final invoiceNumber = detail.invoiceNumber?.toString() ?? 'receipt';
    final text = _receiptText();
    final bytes = Uint8List.fromList(utf8.encode(text));
    try {
      final savedDestination = await FilePicker.saveFile(
        dialogTitle: 'Save receipt text',
        fileName: 'invoice-$invoiceNumber.txt',
        type: FileType.custom,
        allowedExtensions: const ['txt'],
        bytes: bytes,
      );
      if (!context.mounted) {
        return;
      }
      if (savedDestination == null) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Receipt export cancelled • no file was saved.'),
          ),
        );
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Receipt saved • invoice #$invoiceNumber • ${bytes.length} bytes',
          ),
        ),
      );
    } catch (_) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Receipt export could not complete. Verify the selected destination before retrying.',
            ),
          ),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final total = _formatMinorCurrency(detail.totalMinor ?? 0, _currencyCode);
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * 0.82,
        child: ListView(
          padding: const EdgeInsets.fromLTRB(24, 8, 24, 28),
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    'Invoice #${detail.invoiceNumber ?? '—'}',
                    style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                ),
                IconButton(
                  tooltip: 'Save receipt text',
                  onPressed: () => _save(context),
                  icon: const Icon(Icons.save_alt_outlined),
                ),
                IconButton(
                  tooltip: 'Copy receipt text',
                  onPressed: () => _copy(context),
                  icon: const Icon(Icons.content_copy_outlined),
                ),
              ],
            ),
            const SizedBox(height: 4),
            Text(
              detail.finalizedAtUtc == null
                  ? 'Recorded locally'
                  : formatBranchLocalTimestamp(
                      detail.finalizedAtUtc!,
                      branchTimeZone,
                    ),
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 18),
            Card(
              child: Padding(
                padding: const EdgeInsets.all(18),
                child: Column(
                  children: [
                    _ReceiptValueRow(
                      label: 'Fulfillment',
                      value: (detail.fulfillment ?? '—').replaceAll('_', ' '),
                    ),
                    const Divider(height: 24),
                    for (final line in detail.lines)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 12),
                        child: _ReceiptValueRow(
                          label: line.modifierNames.isEmpty
                              ? '${line.displayName} × ${line.quantity}'
                              : '${line.displayName} × ${line.quantity}\n${line.modifierNames.join(' • ')}',
                          value: _formatMinorCurrency(
                            line.lineTotalMinor,
                            _currencyCode,
                          ),
                          detail: _formatMinorCurrency(
                            line.unitPriceMinor,
                            _currencyCode,
                          ),
                        ),
                      ),
                    const Divider(height: 24),
                    _ReceiptValueRow(
                      label: 'Subtotal',
                      value: _formatMinorCurrency(
                        detail.subtotalMinor ?? 0,
                        _currencyCode,
                      ),
                    ),
                    if ((detail.discountMinor ?? 0) > 0) ...[
                      const SizedBox(height: 8),
                      _ReceiptValueRow(
                        label: 'Discount',
                        value: _formatMinorCurrency(
                          detail.discountMinor!,
                          _currencyCode,
                        ),
                      ),
                    ],
                    if ((detail.taxMinor ?? 0) > 0) ...[
                      const SizedBox(height: 8),
                      _ReceiptValueRow(
                        label: 'Tax',
                        value: _formatMinorCurrency(
                          detail.taxMinor!,
                          _currencyCode,
                        ),
                      ),
                    ],
                    const SizedBox(height: 8),
                    _ReceiptValueRow(
                      label: 'Total',
                      value: total,
                      emphasize: true,
                    ),
                    if ((detail.refundedMinor ?? 0) > 0) ...[
                      const SizedBox(height: 8),
                      _ReceiptValueRow(
                        label: 'Refunded',
                        value: _formatMinorCurrency(
                          detail.refundedMinor!,
                          _currencyCode,
                        ),
                      ),
                    ],
                    const Divider(height: 24),
                    for (final payment in detail.payments)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 8),
                        child: _ReceiptValueRow(
                          label: payment.paymentMethod.toUpperCase(),
                          value: _formatMinorCurrency(
                            payment.amountMinor,
                            _currencyCode,
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 12),
            Text(
              'This receipt is reconstructed from immutable order, invoice, and payment snapshots. It is not a printer integration.',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ReceiptValueRow extends StatelessWidget {
  const _ReceiptValueRow({
    required this.label,
    required this.value,
    this.detail,
    this.emphasize = false,
  });

  final String label;
  final String value;
  final String? detail;
  final bool emphasize;

  @override
  Widget build(BuildContext context) {
    final style = emphasize
        ? Theme.of(
            context,
          ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w800)
        : Theme.of(context).textTheme.bodyMedium;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(label, style: style),
              if (detail != null)
                Text(
                  detail!,
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
                ),
            ],
          ),
        ),
        const SizedBox(width: 16),
        Text(value, style: style),
      ],
    );
  }
}

String _formatMinorCurrency(int minorUnits, String currencyCode) {
  final sign = minorUnits < 0 ? '-' : '';
  final absolute = minorUnits.abs();
  final whole = absolute ~/ 100;
  final fraction = (absolute % 100).toString().padLeft(2, '0');
  return '$sign$currencyCode $whole.$fraction';
}

class _FeatureCanvas extends StatelessWidget {
  const _FeatureCanvas({
    required this.icon,
    required this.title,
    required this.description,
  });

  final IconData icon;
  final String title;
  final String description;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 520),
        child: Padding(
          padding: const EdgeInsets.all(28),
          child: Card(
            child: Padding(
              padding: const EdgeInsets.all(30),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    icon,
                    size: 42,
                    color: Theme.of(context).colorScheme.primary,
                  ),
                  const SizedBox(height: 18),
                  Text(
                    title,
                    style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    description,
                    textAlign: TextAlign.center,
                    style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
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
