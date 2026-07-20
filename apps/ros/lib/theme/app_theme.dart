import 'package:flutter/material.dart';

abstract final class AppTheme {
  /// Deep indigo sampled from the GTG mark.
  static const Color brand = Color(0xFF060024);

  /// Brighter indigo for dark surfaces and selected chrome.
  static const Color brandBright = Color(0xFF8B7AE8);

  /// Warm accent kept for contrast against the cool brand family.
  static const Color secondary = Color(0xFFB85C2A);

  static ThemeData light() {
    const canvas = Color(0xFFF7F6FB);
    const ink = Color(0xFF14121F);
    const wash = Color(0xFFEEECF6);
    const line = Color(0xFFD8D5E3);
    // Mid indigo so filled controls stay readable on light surfaces.
    const primary = Color(0xFF2A215C);

    final colors =
        ColorScheme.fromSeed(
          seedColor: brand,
          brightness: Brightness.light,
        ).copyWith(
          surface: Colors.white,
          surfaceContainerHighest: const Color(0xFFF0EEF7),
          primary: primary,
          onPrimary: Colors.white,
          secondary: secondary,
          onSecondary: Colors.white,
          outlineVariant: line,
        );

    return _build(
      colors: colors,
      canvas: canvas,
      ink: ink,
      wash: wash,
      line: line,
      cardFill: Colors.white,
      inputFill: Colors.white,
      textBase: ThemeData.light().textTheme,
    );
  }

  static ThemeData dark() {
    const canvas = Color(0xFF07060F);
    const ink = Color(0xFFE8E6F0);
    const wash = Color(0xFF16132A);
    const line = Color(0xFF2C2940);
    const surface = Color(0xFF12101C);

    final colors =
        ColorScheme.fromSeed(
          seedColor: brandBright,
          brightness: Brightness.dark,
        ).copyWith(
          surface: surface,
          surfaceContainerHighest: const Color(0xFF1C1830),
          primary: brandBright,
          onPrimary: Colors.white,
          secondary: const Color(0xFFD4784A),
          onSecondary: Colors.white,
          outlineVariant: line,
        );

    return _build(
      colors: colors,
      canvas: canvas,
      ink: ink,
      wash: wash,
      line: line,
      cardFill: surface,
      inputFill: const Color(0xFF181528),
      textBase: ThemeData.dark().textTheme,
    );
  }

  static ThemeData _build({
    required ColorScheme colors,
    required Color canvas,
    required Color ink,
    required Color wash,
    required Color line,
    required Color cardFill,
    required Color inputFill,
    required TextTheme textBase,
  }) {
    return ThemeData(
      useMaterial3: true,
      brightness: colors.brightness,
      colorScheme: colors,
      scaffoldBackgroundColor: canvas,
      fontFamily: 'sans-serif',
      textTheme: textBase.apply(bodyColor: ink, displayColor: ink),
      cardTheme: CardThemeData(
        color: cardFill,
        elevation: 0,
        margin: EdgeInsets.zero,
        shape: RoundedRectangleBorder(
          borderRadius: const BorderRadius.all(Radius.circular(20)),
          side: BorderSide(color: line),
        ),
      ),
      appBarTheme: AppBarTheme(
        backgroundColor: canvas,
        foregroundColor: ink,
        elevation: 0,
        surfaceTintColor: Colors.transparent,
      ),
      navigationRailTheme: NavigationRailThemeData(
        backgroundColor: Colors.transparent,
        selectedIconTheme: IconThemeData(color: colors.primary),
        unselectedIconTheme: IconThemeData(color: ink.withValues(alpha: 0.72)),
        selectedLabelTextStyle: TextStyle(
          color: colors.primary,
          fontWeight: FontWeight.w700,
        ),
        unselectedLabelTextStyle: TextStyle(
          color: ink.withValues(alpha: 0.78),
          fontWeight: FontWeight.w600,
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: inputFill,
        border: OutlineInputBorder(
          borderRadius: const BorderRadius.all(Radius.circular(14)),
          borderSide: BorderSide(color: line),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: const BorderRadius.all(Radius.circular(14)),
          borderSide: BorderSide(color: line),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: const BorderRadius.all(Radius.circular(14)),
          borderSide: BorderSide(color: colors.primary, width: 2),
        ),
      ),
      chipTheme: const ChipThemeData(
        side: BorderSide.none,
        shape: StadiumBorder(),
        padding: EdgeInsets.symmetric(horizontal: 8),
      ),
      extensions: <ThemeExtension<dynamic>>[
        // `mint` remains the soft brand wash used by overview cards and badges.
        RestaurantColors(mint: wash, ink: ink),
      ],
    );
  }
}

class RestaurantColors extends ThemeExtension<RestaurantColors> {
  const RestaurantColors({required this.mint, required this.ink});

  final Color mint;
  final Color ink;

  @override
  RestaurantColors copyWith({Color? mint, Color? ink}) {
    return RestaurantColors(mint: mint ?? this.mint, ink: ink ?? this.ink);
  }

  @override
  RestaurantColors lerp(
    covariant ThemeExtension<RestaurantColors>? other,
    double t,
  ) {
    if (other is! RestaurantColors) {
      return this;
    }

    return RestaurantColors(
      mint: Color.lerp(mint, other.mint, t) ?? mint,
      ink: Color.lerp(ink, other.ink, t) ?? ink,
    );
  }
}
