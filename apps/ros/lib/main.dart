import 'package:crypto/crypto.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:path_provider/path_provider.dart';

import 'app.dart';
import 'features/command_center/diagnostics_breadcrumbs.dart';
import 'src/rust/api/simple.dart';
import 'src/rust/frb_generated.dart';

/// Allow-listed fingerprint of an exception *type* only — never the message
/// or stack, which may embed paths.
String diagnosticExceptionTypeFingerprint(Object error) {
  final digest = sha256.convert(error.runtimeType.toString().codeUnits);
  final hex = digest.toString().substring(0, 8);
  return 'type_$hex';
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  var coreStatus =
      'Restaurant Operating System • secure local core needs attention';
  var applicationSupportDirectory = '';
  var workspace = const CommunityWorkspace(
    storageStatus:
        'Local storage needs attention • application support storage is unavailable',
    setupRequired: true,
    categories: [],
    products: [],
    customers: [],
    openDrafts: [],
    kitchenTickets: [],
  );
  CommunityStaffSecurity? staffSecurity;

  try {
    await RustLib.init();
    coreStatus = localCoreStatus();
    final directory = await getApplicationSupportDirectory();
    applicationSupportDirectory = directory.path;

    void recordUiError(Object error, {required String eventCode}) {
      recordDiagnosticBreadcrumb(
        applicationSupportDirectory: applicationSupportDirectory,
        eventCode: eventCode,
        detailCode: diagnosticExceptionTypeFingerprint(error),
      );
    }

    FlutterError.onError = (details) {
      FlutterError.presentError(details);
      recordUiError(
        details.exception,
        eventCode: DiagnosticBreadcrumbs.uiError,
      );
    };
    PlatformDispatcher.instance.onError = (error, stack) {
      recordUiError(error, eventCode: DiagnosticBreadcrumbs.uiZoneError);
      // Leave handling to Flutter so debug tooling still surfaces the failure.
      return false;
    };

    // Resolve the narrow PIN/session state before requesting any operational
    // projection. Rust independently enforces this ordering, but keeping the
    // startup sequence aligned means Dart never intentionally holds customer,
    // order, kitchen, or financial data behind only a visual lock screen.
    staffSecurity = await loadCommunityStaffSecurity(
      applicationSupportDirectory: applicationSupportDirectory,
    );
    workspace = await loadCommunityWorkspace(
      applicationSupportDirectory: applicationSupportDirectory,
    );
  } catch (error) {
    // Start the shell in a safe, non-operational state. This deliberately
    // avoids leaking local paths, keys, or credential-store details while
    // giving a counter user a recoverable screen instead of a blank process.
    if (applicationSupportDirectory.isNotEmpty) {
      await recordDiagnosticBreadcrumb(
        applicationSupportDirectory: applicationSupportDirectory,
        eventCode: DiagnosticBreadcrumbs.uiZoneError,
        detailCode: diagnosticExceptionTypeFingerprint(error),
      );
    }
  }

  runApp(
    RestaurantOperatingSystemApp(
      coreStatus: coreStatus,
      workspace: workspace,
      applicationSupportDirectory: applicationSupportDirectory,
      staffSecurity: staffSecurity,
    ),
  );
}
