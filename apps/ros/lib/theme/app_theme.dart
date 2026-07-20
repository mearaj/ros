import 'package:flutter/material.dart';

abstract final class AppTheme {
  static const Color brand = Color(0xFF126B4A);
  static const Color brandBright = Color(0xFF2F9B6A);
  static const Color secondary = Color(0xFFB85C2A);

  static ThemeData light() {
    const canvas = Color(0xFFF8F9F7);
    const ink = Color(0xFF17211D);
    const mint = Color(0xFFE6F4EC);
    const line = Color(0xFFD9E1DB);

    final colors =
        ColorScheme.fromSeed(
          seedColor: brand,
          brightness: Brightness.light,
        ).copyWith(
          surface: Colors.white,
          surfaceContainerHighest: const Color(0xFFF0F3F0),
          primary: brand,
          onPrimary: Colors.white,
          secondary: secondary,
          onSecondary: Colors.white,
          outlineVariant: line,
        );

    return _build(
      colors: colors,
      canvas: canvas,
      ink: ink,
      mint: mint,
      line: line,
      cardFill: Colors.white,
      inputFill: Colors.white,
      textBase: ThemeData.light().textTheme,
    );
  }

  static ThemeData dark() {
    const canvas = Color(0xFF0F1412);
    const ink = Color(0xFFE6ECE8);
    const mint = Color(0xFF1A2C24);
    const line = Color(0xFF2C3933);
    const surface = Color(0xFF171E1B);

    final colors =
        ColorScheme.fromSeed(
          seedColor: brandBright,
          brightness: Brightness.dark,
        ).copyWith(
          surface: surface,
          surfaceContainerHighest: const Color(0xFF222B27),
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
      mint: mint,
      line: line,
      cardFill: surface,
      inputFill: const Color(0xFF1C2420),
      textBase: ThemeData.dark().textTheme,
    );
  }

  static ThemeData _build({
    required ColorScheme colors,
    required Color canvas,
    required Color ink,
    required Color mint,
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
        RestaurantColors(mint: mint, ink: ink),
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
