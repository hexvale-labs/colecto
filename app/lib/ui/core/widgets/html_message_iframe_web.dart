import 'dart:js_interop';
import 'dart:ui_web' as ui_web;

import 'package:flutter/widgets.dart';
import 'package:web/web.dart' as web;

const _kIframeViewType = 'colecto-html-frame';
bool _factoryRegistered = false;

void _ensureFactoryRegistered() {
  if (_factoryRegistered) return;
  _factoryRegistered = true;
  ui_web.platformViewRegistry.registerViewFactory(_kIframeViewType, (
    int _, {
    Object? params,
  }) {
    final map = params is Map ? params : const <Object?, Object?>{};
    final html = map['html'] as String? ?? '';
    final designed = map['designed'] == true;
    final ink = map['ink'] as String? ?? '#dce3de';
    final link = map['link'] as String? ?? '#a2d6b1';

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
      ..srcdoc = _document(html, designed, ink, link).toJS;
    iframe.style.setProperty('color-scheme', designed ? 'light' : 'normal');
    // Fade in once the frame has painted instead of flashing a blank sheet.
    iframe.onload = ((web.Event _) {
      iframe.style.opacity = '1';
    }).toJS;
    return iframe;
  });
}

/// Renders Rust-sanitized HTML inside a real `<iframe sandbox>` on Flutter Web
/// (`AGENTS.md` rule 5). Uses a single registered platform view factory with
/// `creationParams` so opened emails never accumulate in a global map.
Widget? buildWebSandboxedHtmlIframe(
  String sanitizedHtml, {
  required bool designed,
  required String ink,
  required String link,
  required double height,
}) {
  _ensureFactoryRegistered();
  return SizedBox(
    height: height,
    child: HtmlElementView(
      key: ValueKey(Object.hash(designed, ink, link, sanitizedHtml)),
      viewType: _kIframeViewType,
      creationParams: <String, Object>{
        'html': sanitizedHtml,
        'designed': designed,
        'ink': ink,
        'link': link,
      },
    ),
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
