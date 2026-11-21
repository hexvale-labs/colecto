import 'dart:js_interop';
import 'dart:ui_web' as ui_web;

import 'package:flutter/widgets.dart';
import 'package:web/web.dart' as web;

final Map<String, String> _viewTypes = {};
int _iframeCounter = 0;

/// Renders Rust-sanitized HTML inside a real `<iframe sandbox>` on Flutter Web
/// (`AGENTS.md` rule 5). Each distinct document registers one view factory,
/// so rebuilds reuse the same frame instead of reloading it. Mail that paints
/// its own colours keeps a light sheet; plain HTML takes the reading colours.
Widget? buildWebSandboxedHtmlIframe(
  String sanitizedHtml, {
  required bool designed,
  required String ink,
  required String link,
  required double height,
}) {
  final key = '$designed|$ink|$link|$sanitizedHtml';
  final viewType = _viewTypes.putIfAbsent(key, () {
    final type = 'colecto-html-frame-${++_iframeCounter}';
    ui_web.platformViewRegistry.registerViewFactory(type, (int _) {
      final iframe = web.HTMLIFrameElement()
        ..sandbox.add('allow-popups')
        ..sandbox.add('allow-popups-to-escape-sandbox')
        ..referrerPolicy = 'no-referrer'
        ..title = 'Message content'
        ..style.border = 'none'
        ..style.width = '100%'
        ..style.height = '100%'
        ..style.borderRadius = designed ? '6px' : '0'
        ..style.background = designed ? '#ffffff' : 'transparent'
        ..style.opacity = '0'
        ..style.transition = 'opacity 180ms cubic-bezier(0.23, 1, 0.32, 1)'
        ..srcdoc = _document(sanitizedHtml, designed, ink, link).toJS;
      iframe.style.setProperty('color-scheme', designed ? 'light' : 'normal');
      // Fade in once the frame has painted instead of flashing a blank sheet.
      iframe.onload = ((web.Event _) {
        iframe.style.opacity = '1';
      }).toJS;
      return iframe;
    });
    return type;
  });
  return SizedBox(
    height: height,
    child: HtmlElementView(viewType: viewType),
  );
}

String _document(String html, bool designed, String ink, String link) =>
    '''
<!doctype html>
<html>
<head>
<meta charset="utf-8">
<base target="_blank">
<style>
  html { color-scheme: ${designed ? 'light' : 'dark light'}; }
  body {
    margin: 0;
    padding: ${designed ? '16px' : '2px 0 24px'};
    font: 15.5px/1.68 system-ui, -apple-system, "Segoe UI", sans-serif;
    color: ${designed ? '#111' : ink};
    background: ${designed ? '#fff' : 'transparent'};
    overflow-wrap: anywhere;
  }
  a { color: ${designed ? '#0b57d0' : link}; text-underline-offset: 3px; }
  img { max-width: 100%; height: auto; }
  table { max-width: 100%; }
  blockquote { margin: 0 0 0 4px; padding-left: 12px; border-left: 2px solid rgba(127,127,127,.35); }
  pre, code { font: 13px/1.5 ui-monospace, "SF Mono", monospace; white-space: pre-wrap; }
</style>
</head>
<body>$html</body>
</html>
''';
