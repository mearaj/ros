import 'dart:io';

import '../../src/rust/api/simple.dart';

/// Optional Gotigin diagnostics intake. Empty means cloud share stays local-only.
const diagnosticsShareBaseUrl = String.fromEnvironment(
  'ROS_DIAGNOSTICS_SHARE_URL',
  defaultValue: '',
);

bool get diagnosticsShareEndpointConfigured =>
    diagnosticsShareBaseUrl.trim().isNotEmpty;

/// Uploads an Owner-consented redacted pack. Never called without UI consent.
Future<({bool uploaded, String status})> uploadDiagnosticsSharePack({
  required CommunityDiagnosticsShareResult prepared,
  required String purpose,
}) async {
  final base = diagnosticsShareBaseUrl.trim();
  if (base.isEmpty) {
    return (
      uploaded: false,
      status:
          'Pack is ready on this device. Cloud share is not configured yet — export the pack or wait for Gotigin intake to be enabled.',
    );
  }
  if (!prepared.prepared || prepared.jsonBytes.isEmpty) {
    return (uploaded: false, status: prepared.storageStatus);
  }
  try {
    final uri = Uri.parse(base).replace(path: '/v1/diagnostics/packs');
    final client = HttpClient();
    try {
      final request = await client.postUrl(uri);
      request.headers.contentType = ContentType(
        'application',
        'json',
        charset: 'utf-8',
      );
      request.headers.set('x-ros-diagnostics-schema', '1');
      request.headers.set('x-ros-diagnostics-purpose', purpose);
      request.headers.set(
        'x-ros-diagnostics-consented-at',
        DateTime.now().toUtc().toIso8601String(),
      );
      if (prepared.sha256 != null) {
        request.headers.set('x-ros-diagnostics-sha256', prepared.sha256!);
      }
      request.add(prepared.jsonBytes);
      final response = await request.close();
      final statusCode = response.statusCode;
      await response.drain<void>();
      if (statusCode >= 200 && statusCode < 300) {
        return (
          uploaded: true,
          status: 'Diagnostics pack shared with Gotigin • thank you',
        );
      }
      return (
        uploaded: false,
        status:
            'Share needs attention • the diagnostics intake rejected the pack',
      );
    } finally {
      client.close(force: true);
    }
  } catch (_) {
    return (
      uploaded: false,
      status:
          'Share needs attention • the diagnostics intake could not be reached',
    );
  }
}
