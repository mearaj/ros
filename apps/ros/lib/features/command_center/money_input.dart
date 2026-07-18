/// Converts a two-decimal currency entry to minor units without floating point.
/// The trusted Rust core still validates the currency and persists the amount.
int? parseDecimalPriceToMinorUnits(String value) {
  final normalized = value.trim().replaceAll(',', '');
  final match = RegExp(r'^(\d+)(?:\.(\d{1,2}))?$').firstMatch(normalized);
  if (match == null) {
    return null;
  }

  final fraction = (match.group(2) ?? '').padRight(2, '0');
  final minorUnits =
      BigInt.parse(match.group(1)!) * BigInt.from(100) + BigInt.parse(fraction);
  final maximumI64 = BigInt.parse('9223372036854775807');
  if (minorUnits > maximumI64) {
    return null;
  }

  return minorUnits.toInt();
}

/// Compatibility alias for existing INR-labelled form fields. New code should
/// use [parseDecimalPriceToMinorUnits], which is valid for every currently
/// supported two-decimal Community currency.
int? parseInrPriceToMinorUnits(String value) =>
    parseDecimalPriceToMinorUnits(value);

/// Renders integer minor units for display without introducing a floating-point
/// calculation into the cashier-facing UI.
String formatMinorPrice(int minorUnits, String currencyCode) {
  final absolute = minorUnits.abs();
  final whole = absolute ~/ 100;
  final fraction = (absolute % 100).toString().padLeft(2, '0');
  final sign = minorUnits < 0 ? '-' : '';
  return '$currencyCode $sign$whole.$fraction';
}
