import 'package:flutter_test/flutter_test.dart';
import 'package:ros/features/command_center/diagnostics_breadcrumbs.dart';
import 'package:ros/features/command_center/diagnostics_share.dart';
import 'package:ros/main.dart';

void main() {
  test(
    'breadcrumb codes stay within the local-diagnostics allow-list shape',
    () {
      for (final code in [
        DiagnosticBreadcrumbs.navReports,
        DiagnosticBreadcrumbs.actionBackupVerify,
        DiagnosticBreadcrumbs.actionDiagnosticsOpen,
        DiagnosticBreadcrumbs.uiError,
        DiagnosticBreadcrumbs.actionDayClose,
        DiagnosticBreadcrumbs.actionExportCsv,
      ]) {
        expect(code.contains(RegExp(r'^[a-z0-9_]+$')), isTrue);
        expect(code.split('_'), isNot(contains('pin')));
        expect(code.split('_'), isNot(contains('customer')));
        expect(code.split('_'), isNot(contains('key')));
      }
    },
  );

  test(
    'exception fingerprints are allow-listed detail codes without messages',
    () {
      final fingerprint = diagnosticExceptionTypeFingerprint(
        StateError('secret /tmp/path'),
      );
      expect(fingerprint, matches(RegExp(r'^type_[a-f0-9]{8}$')));
      expect(fingerprint.contains('secret'), isFalse);
      expect(fingerprint.contains('tmp'), isFalse);
    },
  );

  test('cloud share stays fail-closed when intake URL is unset', () async {
    expect(diagnosticsShareEndpointConfigured, isFalse);
  });
}
