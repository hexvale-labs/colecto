import 'dart:typed_data';

import 'file_io_io.dart'
    if (dart.library.js_interop) 'file_io_web.dart'
    as impl;

class PickedAttachmentFile {
  const PickedAttachmentFile({
    required this.name,
    required this.contentType,
    required this.bytes,
  });

  final String name;
  final String contentType;
  final Uint8List bytes;

  int get size => bytes.length;
}

/// Saves a downloaded attachment to disk (native) or triggers a browser download (web).
/// Returns a human-readable destination description (e.g. saved path or filename).
Future<String> saveDownloadedFile({
  required String name,
  required String contentType,
  required Uint8List bytes,
}) =>
    impl.saveDownloadedFile(name: name, contentType: contentType, bytes: bytes);

/// Opens an OS/browser multi-file picker.
/// Returns `null` if no native picker is available on this platform, or an empty
/// list if the user cancelled.
Future<List<PickedAttachmentFile>?> pickAttachmentFiles() =>
    impl.pickAttachmentFiles();

/// Opens an external `http:`, `https:`, or `mailto:` URL in the OS browser/mail handler.
Future<bool> openExternalUrl(String url) => impl.openExternalUrl(url);
