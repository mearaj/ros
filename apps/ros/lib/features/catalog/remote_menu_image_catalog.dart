import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';

/// The catalogue origin is a release-controlled value, never restaurant input.
/// It is deliberately separate from the API's image CDN allow-list.
const remoteMenuImageCatalogOrigin = 'https://ros.gotigin.com';

const _allowedHosts = {'ros.gotigin.com', 'images.gotigin.com'};
const _maximumSearchResponseBytes = 2 * 1024 * 1024;
const _maximumRemoteImageBytes = 16 * 1024 * 1024;
const _maximumImageDimension = 8192;
const _maximumDisplayNameLength = 160;
const _maximumImageIdLength = 128;
const _maximumLicenceLabelLength = 160;
const _maximumTags = 24;
const _maximumTagLength = 48;
const _maximumCursorLength = 512;

class RemoteMenuImage {
  const RemoteMenuImage({
    required this.imageId,
    required this.displayName,
    required this.thumbnailUrl,
    required this.imageUrl,
    required this.contentSha256,
    required this.contentType,
    required this.tags,
    required this.width,
    required this.height,
    required this.licence,
    required this.licenceUrl,
  });

  final String imageId;
  final String displayName;
  final Uri thumbnailUrl;
  final Uri imageUrl;
  final String contentSha256;
  final String contentType;
  final List<String> tags;
  final int width;
  final int height;
  final String licence;
  final Uri licenceUrl;

  factory RemoteMenuImage.fromJson(Map<String, Object?> json) {
    final imageId = _requiredString(json, 'image_id');
    final displayName = _requiredString(json, 'display_name');
    final thumbnailUrl = _trustedImageUri(
      _requiredString(json, 'thumbnail_url'),
    );
    final imageUrl = _trustedImageUri(_requiredString(json, 'image_url'));
    final contentSha256 = _requiredString(json, 'content_sha256');
    final contentType = _requiredString(json, 'content_type').toLowerCase();
    final tags = _requiredStringList(json, 'tags');
    final width = _requiredBoundedDimension(json, 'width');
    final height = _requiredBoundedDimension(json, 'height');
    final licence = _requiredString(json, 'licence');
    final licenceUrl = _trustedLicenceUri(_requiredString(json, 'licence_url'));

    if (imageId.length > _maximumImageIdLength ||
        !RegExp(r'^[A-Za-z0-9._:-]+$').hasMatch(imageId)) {
      throw const RemoteMenuImageCatalogException(
        'Invalid catalogue image ID.',
      );
    }
    if (displayName.length > _maximumDisplayNameLength) {
      throw const RemoteMenuImageCatalogException(
        'Catalogue image name is too long.',
      );
    }
    if (licence.length > _maximumLicenceLabelLength) {
      throw const RemoteMenuImageCatalogException(
        'Catalogue image licence is too long.',
      );
    }
    if (!RegExp(r'^[a-f0-9]{64}$').hasMatch(contentSha256)) {
      throw const RemoteMenuImageCatalogException('Invalid image checksum.');
    }
    if (!{'image/jpeg', 'image/png', 'image/webp'}.contains(contentType)) {
      throw const RemoteMenuImageCatalogException(
        'Unsupported remote image type.',
      );
    }
    if (json['attribution_required'] != false) {
      throw const RemoteMenuImageCatalogException(
        'Catalogue image requires unsupported attribution.',
      );
    }
    return RemoteMenuImage(
      imageId: imageId,
      displayName: displayName,
      thumbnailUrl: thumbnailUrl,
      imageUrl: imageUrl,
      contentSha256: contentSha256,
      contentType: contentType,
      tags: tags,
      width: width,
      height: height,
      licence: licence,
      licenceUrl: licenceUrl,
    );
  }
}

class RemoteMenuImagePage {
  const RemoteMenuImagePage({required this.items, this.nextCursor});

  final List<RemoteMenuImage> items;
  final String? nextCursor;

  factory RemoteMenuImagePage.fromJson(Map<String, Object?> json) {
    if (json['schema_version'] != 1) {
      throw const RemoteMenuImageCatalogException(
        'Unsupported catalogue version.',
      );
    }
    final rawItems = json['items'];
    if (rawItems is! List<Object?> || rawItems.length > 24) {
      throw const RemoteMenuImageCatalogException(
        'Invalid catalogue response.',
      );
    }
    final cursor = json['next_cursor'];
    if (cursor != null &&
        (cursor is! String ||
            cursor.isEmpty ||
            cursor.length > _maximumCursorLength)) {
      throw const RemoteMenuImageCatalogException('Invalid catalogue cursor.');
    }
    return RemoteMenuImagePage(
      items: rawItems
          .map((item) {
            if (item is! Map) {
              throw const RemoteMenuImageCatalogException(
                'Invalid catalogue response.',
              );
            }
            return RemoteMenuImage.fromJson(_stringKeyedMap(item));
          })
          .toList(growable: false),
      nextCursor: cursor as String?,
    );
  }
}

class RemoteMenuImageCatalogClient {
  RemoteMenuImageCatalogClient({HttpClient? httpClient})
    : _httpClient = httpClient ?? HttpClient();

  final HttpClient _httpClient;

  void close() => _httpClient.close(force: true);

  Future<RemoteMenuImagePage> search({
    String query = '',
    String? cursor,
  }) async {
    final normalizedQuery = query.trim();
    if (normalizedQuery.length > 120 ||
        (cursor != null &&
            (cursor.isEmpty || cursor.length > _maximumCursorLength))) {
      throw const RemoteMenuImageCatalogException(
        'Catalogue search is too long.',
      );
    }
    final uri = Uri.parse('$remoteMenuImageCatalogOrigin/v1/menu-images/search')
        .replace(
          queryParameters: {
            'q': normalizedQuery,
            'limit': '24',
            'cursor': ?cursor,
          },
        );
    final response = await _get(uri, maximumBytes: _maximumSearchResponseBytes);
    final contentType = response.contentType?.mimeType.toLowerCase();
    if (contentType != 'application/json') {
      throw const RemoteMenuImageCatalogException(
        'Catalogue returned an invalid response.',
      );
    }
    try {
      final decoded = jsonDecode(utf8.decode(response.bytes));
      if (decoded is! Map) {
        throw const RemoteMenuImageCatalogException(
          'Catalogue returned invalid JSON.',
        );
      }
      return RemoteMenuImagePage.fromJson(_stringKeyedMap(decoded));
    } on FormatException {
      throw const RemoteMenuImageCatalogException(
        'Catalogue returned invalid JSON.',
      );
    }
  }

  /// Downloads the immutable original and verifies the catalogue's digest
  /// before the bytes are sent into Rust's normalisation pipeline.
  Future<Uint8List> downloadVerifiedImage(RemoteMenuImage image) async {
    final response = await _get(
      image.imageUrl,
      maximumBytes: _maximumRemoteImageBytes,
    );
    final responseType = response.contentType?.mimeType.toLowerCase();
    if (responseType != image.contentType) {
      throw const RemoteMenuImageCatalogException(
        'Remote image type did not match its catalogue record.',
      );
    }
    final digest = sha256.convert(response.bytes).toString();
    if (digest != image.contentSha256) {
      throw const RemoteMenuImageCatalogException(
        'Remote image integrity check failed.',
      );
    }
    return response.bytes;
  }

  Future<_RemoteResponse> _get(Uri uri, {required int maximumBytes}) async {
    if (uri.scheme != 'https' || !_allowedHosts.contains(uri.host)) {
      throw const RemoteMenuImageCatalogException(
        'Untrusted catalogue address.',
      );
    }
    try {
      _httpClient.connectionTimeout = const Duration(seconds: 8);
      final request = await _httpClient
          .getUrl(uri)
          .timeout(const Duration(seconds: 10));
      request.followRedirects = false;
      final response = await request.close().timeout(
        const Duration(seconds: 15),
      );
      if (response.statusCode != HttpStatus.ok) {
        throw const RemoteMenuImageCatalogException(
          'Gotigin image catalogue is temporarily unavailable.',
        );
      }
      if (response.contentLength > maximumBytes) {
        throw const RemoteMenuImageCatalogException(
          'Catalogue response is too large.',
        );
      }
      final builder = BytesBuilder(copy: false);
      await for (final chunk in response) {
        builder.add(chunk);
        if (builder.length > maximumBytes) {
          throw const RemoteMenuImageCatalogException(
            'Catalogue response is too large.',
          );
        }
      }
      return _RemoteResponse(
        bytes: builder.takeBytes(),
        contentType: response.headers.contentType,
      );
    } on RemoteMenuImageCatalogException {
      rethrow;
    } on SocketException {
      throw const RemoteMenuImageCatalogException(
        'No connection to the Gotigin image catalogue.',
      );
    } on TimeoutException {
      throw const RemoteMenuImageCatalogException(
        'Gotigin image catalogue timed out.',
      );
    } on HttpException {
      throw const RemoteMenuImageCatalogException(
        'Gotigin image catalogue could not be reached.',
      );
    } on Object {
      throw const RemoteMenuImageCatalogException(
        'Gotigin image catalogue could not be reached.',
      );
    }
  }
}

class _RemoteResponse {
  const _RemoteResponse({required this.bytes, required this.contentType});

  final Uint8List bytes;
  final ContentType? contentType;
}

class RemoteMenuImageCatalogException implements Exception {
  const RemoteMenuImageCatalogException(this.message);

  final String message;

  @override
  String toString() => message;
}

String _requiredString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! String ||
      value.isEmpty ||
      value != value.trim() ||
      value.runes.any((rune) => rune < 0x20 || rune == 0x7f)) {
    throw const RemoteMenuImageCatalogException('Invalid catalogue response.');
  }
  return value;
}

List<String> _requiredStringList(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! List<Object?> || value.length > _maximumTags) {
    throw const RemoteMenuImageCatalogException('Invalid catalogue tags.');
  }
  final tags = <String>[];
  for (final item in value) {
    if (item is! String ||
        item.isEmpty ||
        item != item.trim() ||
        item.runes.any((rune) => rune < 0x20 || rune == 0x7f) ||
        item.length > _maximumTagLength) {
      throw const RemoteMenuImageCatalogException('Invalid catalogue tags.');
    }
    tags.add(item);
  }
  return List.unmodifiable(tags);
}

int _requiredBoundedDimension(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! int || value <= 0 || value > _maximumImageDimension) {
    throw const RemoteMenuImageCatalogException(
      'Invalid catalogue image dimensions.',
    );
  }
  return value;
}

Map<String, Object?> _stringKeyedMap(Object value) {
  if (value is! Map || value.keys.any((key) => key is! String)) {
    throw const RemoteMenuImageCatalogException('Invalid catalogue response.');
  }
  return Map<String, Object?>.from(value);
}

Uri _trustedImageUri(String value) {
  final uri = Uri.tryParse(value);
  if (uri == null ||
      uri.scheme != 'https' ||
      !_allowedHosts.contains(uri.host) ||
      uri.userInfo.isNotEmpty ||
      uri.hasFragment) {
    throw const RemoteMenuImageCatalogException(
      'Untrusted catalogue image address.',
    );
  }
  return uri;
}

Uri _trustedLicenceUri(String value) {
  final uri = Uri.tryParse(value);
  if (uri == null ||
      uri.scheme != 'https' ||
      uri.host.isEmpty ||
      uri.userInfo.isNotEmpty ||
      uri.hasFragment) {
    throw const RemoteMenuImageCatalogException('Invalid image licence URL.');
  }
  return uri;
}
