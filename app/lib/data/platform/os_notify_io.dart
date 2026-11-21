import 'dart:io';

Future<void> sendOsNotification({
  required String title,
  required String body,
}) async {
  try {
    if (Platform.isLinux) {
      await Process.start('notify-send', ['--app-name=Colecto', title, body]);
    } else if (Platform.isMacOS) {
      final safeTitle = title.replaceAll('"', r'\"');
      final safeBody = body.replaceAll('"', r'\"');
      await Process.start('osascript', [
        '-e',
        'display notification "$safeBody" with title "$safeTitle"',
      ]);
    }
  } catch (_) {
    // Best-effort OS notification; ignore when desktop notifier is absent.
  }
}
