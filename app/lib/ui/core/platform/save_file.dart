import 'package:file_selector/file_selector.dart';
import 'package:flutter/foundation.dart';

/// Saves downloaded bytes where the platform expects them: a browser
/// download on the web, a native Save dialog on desktop. Returns false when
/// the person cancels. Throws [UnsupportedError] on phones, which need a
/// share sheet that is not wired in yet.
Future<bool> saveBytes({
  required String name,
  required String contentType,
  required Uint8List bytes,
}) async {
  final file = XFile.fromData(bytes, name: name, mimeType: contentType);
  if (kIsWeb) {
    await file.saveTo(name);
    return true;
  }
  if (defaultTargetPlatform == TargetPlatform.android ||
      defaultTargetPlatform == TargetPlatform.iOS) {
    throw UnsupportedError(
      'Saving attachments is not available on phones yet.',
    );
  }
  final location = await getSaveLocation(suggestedName: name);
  if (location == null) return false;
  await file.saveTo(location.path);
  return true;
}
