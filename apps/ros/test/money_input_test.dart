import 'package:flutter_test/flutter_test.dart';
import 'package:ros/features/command_center/money_input.dart';

void main() {
  group('parseInrPriceToMinorUnits', () {
    test('preserves paise exactly without floating point', () {
      expect(parseInrPriceToMinorUnits('125.05'), 12_505);
      expect(parseInrPriceToMinorUnits('0.1'), 10);
      expect(parseInrPriceToMinorUnits('1,000.50'), 100_050);
    });

    test('rejects ambiguous, negative, and overflowing amounts', () {
      expect(parseInrPriceToMinorUnits('12.345'), isNull);
      expect(parseInrPriceToMinorUnits('-1'), isNull);
      expect(parseInrPriceToMinorUnits(''), isNull);
      expect(parseInrPriceToMinorUnits('92233720368547758.08'), isNull);
    });
  });

  group('formatMinorPrice', () {
    test('renders integer minor units without a floating-point conversion', () {
      expect(formatMinorPrice(12_505, 'INR'), 'INR 125.05');
      expect(formatMinorPrice(10, 'INR'), 'INR 0.10');
      expect(formatMinorPrice(-50, 'INR'), 'INR -0.50');
    });
  });
}
