import 'dart:io';

import 'package:flutter/material.dart';

/// Persists [ThemeMode] beside the encrypted workspace so appearance survives
/// restarts without involving the Rust operational core.
abstract final class AppearanceStore {
  static const fileName = 'appearance_theme_mode.txt';

  static Future<ThemeMode> load(String applicationSupportDirectory) async {
    if (applicationSupportDirectory.isEmpty) {
      return ThemeMode.light;
    }

    try {
      final file = File('$applicationSupportDirectory/$fileName');
      if (!await file.exists()) {
        return ThemeMode.light;
      }

      return parse((await file.readAsString()).trim());
    } catch (_) {
      return ThemeMode.light;
    }
  }

  static Future<void> save(
    String applicationSupportDirectory,
    ThemeMode mode,
  ) async {
    if (applicationSupportDirectory.isEmpty) {
      return;
    }

    try {
      final file = File('$applicationSupportDirectory/$fileName');
      await file.writeAsString(encode(mode));
    } catch (_) {
      // Appearance is best-effort; keep the in-session choice even if write fails.
    }
  }

  static ThemeMode parse(String value) => switch (value) {
    'dark' => ThemeMode.dark,
    'system' => ThemeMode.system,
    _ => ThemeMode.light,
  };

  static String encode(ThemeMode mode) => switch (mode) {
    ThemeMode.dark => 'dark',
    ThemeMode.system => 'system',
    ThemeMode.light => 'light',
  };
}

class AppAppearance extends InheritedWidget {
  const AppAppearance({
    required this.themeMode,
    required this.onThemeModeChanged,
    required super.child,
    super.key,
  });

  final ThemeMode themeMode;
  final ValueChanged<ThemeMode> onThemeModeChanged;

  static AppAppearance of(BuildContext context) {
    final result = context.dependOnInheritedWidgetOfExactType<AppAppearance>();
    assert(result != null, 'AppAppearance not found in context');
    return result!;
  }

  static AppAppearance? maybeOf(BuildContext context) {
    return context.dependOnInheritedWidgetOfExactType<AppAppearance>();
  }

  @override
  bool updateShouldNotify(AppAppearance oldWidget) {
    return themeMode != oldWidget.themeMode;
  }
}

class AppearanceMenuButton extends StatelessWidget {
  const AppearanceMenuButton({super.key});

  @override
  Widget build(BuildContext context) {
    final appearance = AppAppearance.maybeOf(context);
    if (appearance == null) {
      return const SizedBox.shrink();
    }

    return PopupMenuButton<ThemeMode>(
      key: const Key('appearance-menu'),
      tooltip: 'Appearance',
      initialValue: appearance.themeMode,
      onSelected: appearance.onThemeModeChanged,
      itemBuilder: (context) => const [
        PopupMenuItem(value: ThemeMode.light, child: Text('Light')),
        PopupMenuItem(value: ThemeMode.dark, child: Text('Dark')),
        PopupMenuItem(value: ThemeMode.system, child: Text('System')),
      ],
      icon: Icon(_iconFor(appearance.themeMode)),
    );
  }

  static IconData _iconFor(ThemeMode mode) => switch (mode) {
    ThemeMode.dark => Icons.dark_mode_outlined,
    ThemeMode.system => Icons.brightness_auto_outlined,
    ThemeMode.light => Icons.light_mode_outlined,
  };
}
