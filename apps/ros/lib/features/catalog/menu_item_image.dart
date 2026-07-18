import 'dart:typed_data';

import 'package:flutter/material.dart';

/// Displays a compact menu image while keeping a reliable visual fallback.
///
/// App photos are immutable bundled assets. Restaurant photos arrive from the
/// encrypted local catalog as already-normalised JPEG bytes, so this widget
/// never needs access to a user-selected file path.
class MenuItemImage extends StatelessWidget {
  const MenuItemImage({
    this.assetKey,
    this.imageBytes,
    this.fallbackIcon = Icons.restaurant_outlined,
    this.borderRadius,
    this.fit = BoxFit.cover,
    this.cacheWidth,
    this.cacheHeight,
    super.key,
  });

  final String? assetKey;
  final Uint8List? imageBytes;
  final IconData fallbackIcon;
  final BorderRadius? borderRadius;
  final BoxFit fit;
  final int? cacheWidth;
  final int? cacheHeight;

  @override
  Widget build(BuildContext context) {
    final image = imageBytes != null
        ? Image.memory(
            imageBytes!,
            fit: fit,
            cacheWidth: cacheWidth,
            cacheHeight: cacheHeight,
            excludeFromSemantics: true,
            errorBuilder: (_, _, _) => _fallback(context),
          )
        : assetKey != null
        ? Image.asset(
            menuItemAssetPath(assetKey!),
            fit: fit,
            cacheWidth: cacheWidth,
            cacheHeight: cacheHeight,
            excludeFromSemantics: true,
            errorBuilder: (_, _, _) => _fallback(context),
          )
        : _fallback(context);

    final content = SizedBox.expand(child: image);
    return borderRadius == null
        ? content
        : ClipRRect(borderRadius: borderRadius!, child: content);
  }

  Widget _fallback(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return ColoredBox(
      color: colorScheme.primaryContainer,
      child: Center(
        child: Icon(fallbackIcon, color: colorScheme.primary, size: 30),
      ),
    );
  }
}

String menuItemAssetPath(String assetKey) => 'assets/menu/$assetKey.webp';
