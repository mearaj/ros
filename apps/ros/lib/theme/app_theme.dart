import 'package:flutter/material.dart';

abstract final class AppTheme {
  static ThemeData light() {
    const canvas = Color(0xFFF8F9F7);
    const ink = Color(0xFF17211D);
    const evergreen = Color(0xFF126B4A);
    const mint = Color(0xFFE6F4EC);
    const line = Color(0xFFD9E1DB);

    final colors =
        ColorScheme.fromSeed(
          seedColor: evergreen,
          brightness: Brightness.light,
        ).copyWith(
          surface: Colors.white,
          surfaceContainerHighest: const Color(0xFFF0F3F0),
          primary: evergreen,
          onPrimary: Colors.white,
          secondary: const Color(0xFFB85C2A),
          onSecondary: Colors.white,
          outlineVariant: line,
        );

    return ThemeData(
      useMaterial3: true,
      colorScheme: colors,
      scaffoldBackgroundColor: canvas,
      fontFamily: 'sans-serif',
      textTheme: ThemeData.light().textTheme.apply(
        bodyColor: ink,
        displayColor: ink,
      ),
      cardTheme: const CardThemeData(
        color: Colors.white,
        elevation: 0,
        margin: EdgeInsets.zero,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.all(Radius.circular(20)),
          side: BorderSide(color: line),
        ),
      ),
      appBarTheme: const AppBarTheme(
        backgroundColor: canvas,
        foregroundColor: ink,
        elevation: 0,
        surfaceTintColor: Colors.transparent,
      ),
      inputDecorationTheme: const InputDecorationTheme(
        filled: true,
        fillColor: Colors.white,
        border: OutlineInputBorder(
          borderRadius: BorderRadius.all(Radius.circular(14)),
          borderSide: BorderSide(color: line),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.all(Radius.circular(14)),
          borderSide: BorderSide(color: line),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.all(Radius.circular(14)),
          borderSide: BorderSide(color: evergreen, width: 2),
        ),
      ),
      chipTheme: const ChipThemeData(
        side: BorderSide.none,
        shape: StadiumBorder(),
        padding: EdgeInsets.symmetric(horizontal: 8),
      ),
      extensions: const <ThemeExtension<dynamic>>[
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
