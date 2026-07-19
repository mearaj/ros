import 'package:flutter/material.dart';

import 'features/command_center/restaurant_shell.dart';
import 'src/rust/api/simple.dart';
import 'theme/app_theme.dart';

class RestaurantOperatingSystemApp extends StatelessWidget {
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
    super.key,
  });

  final String coreStatus;
  final CommunityWorkspace workspace;
  final String applicationSupportDirectory;
  final CommunityStaffSecurity? staffSecurity;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'Restaurant Operating System',
      theme: AppTheme.light(),
      // Every status, error, and report text in the app must be selectable so
      // an owner or installer can copy the exact diagnostic wording. Editable
      // fields keep their own selection behavior; buttons are unaffected.
      // SelectionArea needs an Overlay ancestor for its selection handles and
      // context menu, so it is hosted in a dedicated Overlay above the
      // Navigator. This covers routes, dialogs, and snack bars alike.
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
        coreStatus: coreStatus,
        workspace: workspace,
        applicationSupportDirectory: applicationSupportDirectory,
        staffSecurity: staffSecurity,
      ),
    );
  }
}
