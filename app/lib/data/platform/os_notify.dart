import 'os_notify_io.dart'
    if (dart.library.js_interop) 'os_notify_web.dart'
    as impl;

/// Sends a best-effort native OS or browser notification.
Future<void> sendOsNotification({
  required String title,
  required String body,
}) => impl.sendOsNotification(title: title, body: body);
