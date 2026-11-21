import 'package:web/web.dart' as web;

Future<bool> openExternalUrl(String url) async {
  final uri = Uri.tryParse(url.trim());
  if (uri == null ||
      (uri.scheme != 'http' &&
          uri.scheme != 'https' &&
          uri.scheme != 'mailto')) {
    return false;
  }
  web.window.open(uri.toString(), '_blank', 'noopener,noreferrer');
  return true;
}
