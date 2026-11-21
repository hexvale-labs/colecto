import 'dart:io';

Future<bool> openExternalUrl(String url) async {
  final uri = Uri.tryParse(url.trim());
  if (uri == null ||
      (uri.scheme != 'http' &&
          uri.scheme != 'https' &&
          uri.scheme != 'mailto')) {
    return false;
  }
  try {
    if (Platform.isLinux) {
      await Process.start('xdg-open', [uri.toString()]);
      return true;
    }
    if (Platform.isMacOS) {
      await Process.start('open', [uri.toString()]);
      return true;
    }
    if (Platform.isWindows) {
      // Never invoke `cmd /c start`, which interprets `&`, `|`, `^`, and `%`.
      await Process.start('rundll32', [
        'url.dll,FileProtocolHandler',
        uri.toString(),
      ]);
      return true;
    }
  } catch (_) {}
  return false;
}
