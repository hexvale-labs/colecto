import 'package:flutter/widgets.dart';

/// Non-web stub for sandboxed HTML iframe rendering.
/// On desktop/mobile, `HtmlMessageView` renders the message's text natively
/// until the pooled platform WebView spike is wired in.
Widget? buildWebSandboxedHtmlIframe(
  String sanitizedHtml, {
  required bool designed,
  required String ink,
  required String link,
  required double height,
}) => null;
