/// Presentation-only conversion of UTC timestamps into the branch time zone.
///
/// Accounting-day boundaries remain UTC (`YYYY-MM-DD` from `finalized_at_utc`).
/// This helper never changes stored facts or day scoping.
String formatBranchLocalTimestamp(String utcIso, [String? timeZoneId]) {
  final parsed = DateTime.tryParse(utcIso);
  if (parsed == null) {
    return utcIso;
  }
  final utc = parsed.toUtc();
  final zone = timeZoneId?.trim() ?? '';
  final offset = fixedUtcOffsetForTimeZone(zone);
  if (offset == null) {
    final stamped = _formatUtcClock(utc);
    return zone.isEmpty ? '$stamped UTC' : '$stamped UTC ($zone)';
  }
  final local = utc.add(offset);
  final label = shortTimeZoneLabel(zone);
  return '${_formatLocalClock(local)} $label';
}

/// Fixed offsets for zones used by Community setup. Zones with DST fall back
/// to an explicit UTC label so the UI never invents an ambiguous local time.
Duration? fixedUtcOffsetForTimeZone(String timeZoneId) {
  switch (timeZoneId) {
    case 'Asia/Kolkata':
    case 'Asia/Calcutta':
      return const Duration(hours: 5, minutes: 30);
    case 'Asia/Dubai':
      return const Duration(hours: 4);
    case 'Asia/Singapore':
    case 'Asia/Kuala_Lumpur':
      return const Duration(hours: 8);
    case 'UTC':
    case 'Etc/UTC':
    case 'Etc/GMT':
      return Duration.zero;
    default:
      return null;
  }
}

String shortTimeZoneLabel(String timeZoneId) {
  switch (timeZoneId) {
    case 'Asia/Kolkata':
    case 'Asia/Calcutta':
      return 'IST';
    case 'Asia/Dubai':
      return 'GST';
    case 'Asia/Singapore':
    case 'Asia/Kuala_Lumpur':
      return 'SGT';
    case 'UTC':
    case 'Etc/UTC':
    case 'Etc/GMT':
      return 'UTC';
    default:
      return timeZoneId;
  }
}

String _formatUtcClock(DateTime utc) {
  final iso = utc.toIso8601String();
  final withoutZ = iso.endsWith('Z') ? iso.substring(0, iso.length - 1) : iso;
  return withoutZ.replaceFirst('T', ' ');
}

String _formatLocalClock(DateTime local) {
  final y = local.year.toString().padLeft(4, '0');
  final m = local.month.toString().padLeft(2, '0');
  final d = local.day.toString().padLeft(2, '0');
  final h = local.hour.toString().padLeft(2, '0');
  final min = local.minute.toString().padLeft(2, '0');
  final s = local.second.toString().padLeft(2, '0');
  final ms = local.millisecond.toString().padLeft(3, '0');
  return '$y-$m-$d $h:$min:$s.$ms';
}
