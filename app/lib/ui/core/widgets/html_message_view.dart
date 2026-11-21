import 'package:material_ui/material_ui.dart';

import '../theme/colecto_palette.dart';
import 'html_message_iframe_stub.dart'
    if (dart.library.js_interop) 'html_message_iframe_web.dart';
import 'html_message_native.dart';

/// Displays Rust-sanitized HTML email:
/// - On Web, inside a real `<iframe sandbox>` via `HtmlElementView`, created
///   once per document and faded in after it paints.
/// - On Desktop/Mobile, as the message's text rendered natively (zero
///   WebView RAM) until the pooled platform WebView is wired in.
class HtmlMessageView extends StatelessWidget {
  const HtmlMessageView({
    super.key,
    required this.html,
    required this.plainTextFallback,
    required this.height,
  });

  final String html;
  final String plainTextFallback;
  final double height;

  /// Mail that paints its own backgrounds or text colours keeps its own
  /// light sheet, so dark ink stays readable.
  static bool isDesigned(String html) => RegExp(
    r'bgcolor\s*=|background(-color)?\s*:|(^|[;\s"])color\s*:',
    caseSensitive: false,
  ).hasMatch(html);

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final frame = buildWebSandboxedHtmlIframe(
      html,
      designed: isDesigned(html),
      ink: ColectoPalette.toHex(p.bodyInk),
      link: ColectoPalette.toHex(p.accent),
      height: height,
    );
    if (frame != null) return frame;
    return NativeSanitizedHtmlView(
      html: html,
      plainTextFallback: plainTextFallback,
    );
  }

  static String stripHtml(String input) {
    return input
        .replaceAll(RegExp(r'<br\s*/?>', caseSensitive: false), '\n')
        .replaceAll(
          RegExp(r'</(p|div|h[1-6]|li|tr)>', caseSensitive: false),
          '\n\n',
        )
        .replaceAll(RegExp(r'<[^>]+>'), '')
        .replaceAll('&nbsp;', ' ')
        .replaceAll('&amp;', '&')
        .replaceAll('&lt;', '<')
        .replaceAll('&gt;', '>')
        .replaceAll('&quot;', '"')
        .replaceAll('&#39;', "'")
        .replaceAll(RegExp(r'\n{3,}'), '\n\n')
        .trim();
  }
}
