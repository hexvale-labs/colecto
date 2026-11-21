import 'package:web/web.dart' as web;

Future<void> sendOsNotification({
  required String title,
  required String body,
}) async {
  try {
    if (web.Notification.permission == 'granted') {
      web.Notification(title, web.NotificationOptions(body: body));
    }
  } catch (_) {
    // Browser Notification API unavailable or blocked.
  }
}
