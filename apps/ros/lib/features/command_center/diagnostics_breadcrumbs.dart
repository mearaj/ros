import '../../src/rust/api/simple.dart';

/// Allow-listed UI breadcrumb codes. Rust rejects anything outside the
/// local-diagnostics-v1 allow-list.
abstract final class DiagnosticBreadcrumbs {
  static const navOverview = 'nav_overview';
  static const navPos = 'nav_pos';
  static const navKitchen = 'nav_kitchen';
  static const navInventory = 'nav_inventory';
  static const navReports = 'nav_reports';
  static const actionBackupCreate = 'action_backup_create';
  static const actionBackupVerify = 'action_backup_verify';
  static const actionBackupRestore = 'action_backup_restore';
  static const actionDayClose = 'action_day_close';
  static const actionExportCsv = 'action_export_csv';
  static const actionDiagnosticsOpen = 'action_diagnostics_open';
  static const uiError = 'ui_error';
  static const uiZoneError = 'ui_zone_error';
}

Future<void> recordDiagnosticBreadcrumb({
  required String applicationSupportDirectory,
  required String eventCode,
  String? detailCode,
}) async {
  if (applicationSupportDirectory.isEmpty) {
    return;
  }
  try {
    await recordCommunityDiagnosticBreadcrumb(
      applicationSupportDirectory: applicationSupportDirectory,
      eventCode: eventCode,
      detailCode: detailCode,
    );
  } catch (_) {
    // Diagnostics must never break cashier flows.
  }
}
