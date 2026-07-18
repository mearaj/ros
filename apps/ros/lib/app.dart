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
      home: RestaurantShell(
        coreStatus: coreStatus,
        workspace: workspace,
        applicationSupportDirectory: applicationSupportDirectory,
        staffSecurity: staffSecurity,
      ),
    );
  }
}
