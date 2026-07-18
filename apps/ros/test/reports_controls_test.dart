import 'package:flutter_test/flutter_test.dart';
import 'package:ros/src/rust/api/simple.dart';

void main() {
  test('sales summary carries day-scoped tax, discount, and close state', () {
    const summary = CommunitySalesSummary(
      storageStatus: 'Local report verified',
      available: true,
      accountingDateUtc: '2026-07-17',
      branchTimeZone: 'Asia/Kolkata',
      invoiceCount: 2,
      totalMinor: 9900,
      cashMinor: 9900,
      cardMinor: 0,
      upiMinor: 0,
      refundMinor: 0,
      expenseMinor: 500,
      discountMinor: 1000,
      taxMinor: 900,
      currencyCode: 'INR',
      schemaVersion: 30,
      auditEventCount: 12,
      dayClosed: true,
      dayClosedAtUtc: '2026-07-17T20:00:00.000Z',
      dayCloseReason: 'End of service',
      recentInvoices: [],
      topItems: [],
    );

    expect(summary.discountMinor, 1000);
    expect(summary.taxMinor, 900);
    expect(summary.dayClosed, isTrue);
    expect(summary.branchTimeZone, 'Asia/Kolkata');
    expect(summary.dayCloseReason, 'End of service');
  });

  test('backup verify result stays distinct from create/restore', () {
    const verified = CommunityBackupResult(
      storageStatus: 'Verified local backup • schema v30 • 4096 bytes',
      created: false,
      backupFileName: 'restaurant-os-backup-demo.db',
      sha256: 'abc123',
    );
    expect(verified.created, isFalse);
    expect(verified.backupFileName, contains('backup'));
  });
}
