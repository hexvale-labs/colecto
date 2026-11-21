import 'file_io_io.dart'
    if (dart.library.js_interop) 'file_io_web.dart'
    as impl;

/// Opens an external `http:`, `https:`, or `mailto:` URL in the OS browser/mail handler.
Future<bool> openExternalUrl(String url) => impl.openExternalUrl(url);
