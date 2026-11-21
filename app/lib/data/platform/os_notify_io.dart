import 'dart:io';

String _escapeAppleScript(String input) => input
    .replaceAll(r'\', r'\\')
    .replaceAll('"', r'\"')
    .replaceAll('\r', ' ')
    .replaceAll('\n', ' ');

Future<void> sendOsNotification({
  required String title,
  required String body,
}) async {
  try {
    if (Platform.isLinux) {
      await Process.start('notify-send', [
        '--app-name=Colecto',
        '--',
        title,
        body,
      ]);
    } else if (Platform.isMacOS) {
      final safeTitle = _escapeAppleScript(title);
      final safeBody = _escapeAppleScript(body);
      await Process.start('osascript', [
        '-e',
        'display notification "$safeBody" with title "$safeTitle"',
      ]);
    }
  } catch (_) {
    // Best-effort OS notification; ignore when desktop notifier is absent.
  }
}
