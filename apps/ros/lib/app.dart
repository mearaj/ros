import 'package:flutter/material.dart';

import 'features/command_center/restaurant_shell.dart';
import 'src/rust/api/simple.dart';
import 'theme/app_theme.dart';
import 'theme/appearance.dart';

class RestaurantOperatingSystemApp extends StatefulWidget {
  const RestaurantOperatingSystemApp({
    required this.coreStatus,
    this.workspace = const CommunityWorkspace(
      storageStatus: 'Encrypted local storage ready',
      setupRequired: false,
      categories: [],
      products: [],
      customers: [],
      openDrafts: [],
      kitchenTickets: [],
    ),
    this.applicationSupportDirectory = '',
    this.staffSecurity,
    this.initialThemeMode = ThemeMode.light,
    super.key,
  });

  final String coreStatus;
  final CommunityWorkspace workspace;
  final String applicationSupportDirectory;
  final CommunityStaffSecurity? staffSecurity;

  /// Used by tests; production loads the saved preference in [initState].
  final ThemeMode initialThemeMode;

  @override
  State<RestaurantOperatingSystemApp> createState() =>
      _RestaurantOperatingSystemAppState();
}

class _RestaurantOperatingSystemAppState
    extends State<RestaurantOperatingSystemApp> {
  late ThemeMode _themeMode = widget.initialThemeMode;

  @override
  void initState() {
    super.initState();
    _restoreAppearance();
  }

  Future<void> _restoreAppearance() async {
    final mode = await AppearanceStore.load(widget.applicationSupportDirectory);
    if (!mounted || mode == _themeMode) {
      return;
    }
    setState(() => _themeMode = mode);
  }

  Future<void> _setThemeMode(ThemeMode mode) async {
    if (mode == _themeMode) {
      return;
    }
    setState(() => _themeMode = mode);
    await AppearanceStore.save(widget.applicationSupportDirectory, mode);
  }

  @override
  Widget build(BuildContext context) {
    return AppAppearance(
      themeMode: _themeMode,
      onThemeModeChanged: _setThemeMode,
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        title: 'Restaurant Operating System',
        theme: AppTheme.light(),
        darkTheme: AppTheme.dark(),
        themeMode: _themeMode,
        // Visible text is selectable by default so staff can copy prices,
        // statuses, errors, and report wording. Editable fields keep their own
        // selection. Interactive chrome (buttons, nav destinations, PIN keys)
        // may disable selection only where drag-select would block the action.
        // MaterialApp.builder sits above the Navigator Overlay, so SelectionArea
        // needs its own Overlay ancestor for handles and the context menu. That
        // still covers routes, dialogs, and snack bars under the same tree.
        builder: (context, child) {
          if (child == null) {
            return const SizedBox.shrink();
          }
          return Overlay(
            initialEntries: [
              OverlayEntry(builder: (context) => SelectionArea(child: child)),
            ],
          );
        },
        home: RestaurantShell(
          coreStatus: widget.coreStatus,
          workspace: widget.workspace,
          applicationSupportDirectory: widget.applicationSupportDirectory,
          staffSecurity: widget.staffSecurity,
        ),
      ),
    );
  }
}
