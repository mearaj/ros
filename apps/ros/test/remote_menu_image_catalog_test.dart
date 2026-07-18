import 'package:flutter_test/flutter_test.dart';
import 'package:ros/features/catalog/remote_menu_image_catalog.dart';

void main() {
  group('RemoteMenuImage', () {
    test('accepts the bounded no-attribution service contract', () {
      final image = RemoteMenuImage.fromJson(_validImage());

      expect(image.imageId, '01JTESTIMAGE');
      expect(image.displayName, 'Chicken biryani');
      expect(image.contentType, 'image/webp');
      expect(image.width, 1600);
      expect(image.height, 1200);
      expect(image.tags, ['indian', 'rice']);
      expect(image.licence, 'Pexels License');
      expect(image.licenceUrl.scheme, 'https');
    });

    test('rejects assets that require attribution', () {
      expect(
        () => RemoteMenuImage.fromJson(
          _validImage()..['attribution_required'] = true,
        ),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
    });

    test('rejects unbounded dimensions and malformed provenance', () {
      expect(
        () => RemoteMenuImage.fromJson(_validImage()..['width'] = 8193),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
      expect(
        () => RemoteMenuImage.fromJson(
          _validImage()..['licence_url'] = 'http://example.com/licence',
        ),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
      expect(
        () => RemoteMenuImage.fromJson(
          _validImage()..['content_sha256'] = 'A' * 64,
        ),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
      expect(
        () => RemoteMenuImage.fromJson(_validImage()..['licence'] = 'x' * 161),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
      expect(
        () => RemoteMenuImage.fromJson(
          _validImage()..['display_name'] = ' Chicken biryani',
        ),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
      expect(
        () => RemoteMenuImage.fromJson(
          _validImage()..['tags'] = <Object?>['indian\nfood'],
        ),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
    });

    test('rejects image URLs outside the release allow-list', () {
      expect(
        () => RemoteMenuImage.fromJson(
          _validImage()
            ..['image_url'] = 'https://attacker.example/menu/source.webp',
        ),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
      expect(
        () => RemoteMenuImage.fromJson(
          _validImage()
            ..['image_url'] =
                'https://user:secret@images.gotigin.com/menu/source.webp',
        ),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
    });
  });

  group('RemoteMenuImagePage', () {
    test('rejects oversized pages and cursors', () {
      expect(
        () => RemoteMenuImagePage.fromJson({
          'schema_version': 1,
          'next_cursor': null,
          'items': List.generate(25, (_) => _validImage()),
        }),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
      expect(
        () => RemoteMenuImagePage.fromJson({
          'schema_version': 1,
          'next_cursor': 'x' * 513,
          'items': <Object?>[],
        }),
        throwsA(isA<RemoteMenuImageCatalogException>()),
      );
    });
  });
}

Map<String, Object?> _validImage() => {
  'image_id': '01JTESTIMAGE',
  'display_name': 'Chicken biryani',
  'tags': <Object?>['indian', 'rice'],
  'thumbnail_url': 'https://images.gotigin.com/menu/chicken-biryani/thumb.webp',
  'image_url': 'https://images.gotigin.com/menu/chicken-biryani/source.webp',
  'content_sha256': 'a' * 64,
  'content_type': 'image/webp',
  'width': 1600,
  'height': 1200,
  'licence': 'Pexels License',
  'licence_url': 'https://www.pexels.com/legal-pages/license/',
  'attribution_required': false,
};
