import 'package:flutter_test/flutter_test.dart';
import 'package:ros/features/command_center/branch_time.dart';

void main() {
  test('formats Asia/Kolkata timestamps in IST without changing UTC day', () {
    expect(
      formatBranchLocalTimestamp('2026-07-17T18:30:00.000Z', 'Asia/Kolkata'),
      '2026-07-18 00:00:00.000 IST',
    );
  });

  test('falls back to labeled UTC for unknown IANA zones', () {
    expect(
      formatBranchLocalTimestamp(
        '2026-07-17T12:00:00.000Z',
        'America/New_York',
      ),
      '2026-07-17 12:00:00.000 UTC (America/New_York)',
    );
  });

  test('keeps UTC when no branch zone is provided', () {
    expect(
      formatBranchLocalTimestamp('2026-07-17T12:00:00.000Z'),
      '2026-07-17 12:00:00.000 UTC',
    );
  });
}
