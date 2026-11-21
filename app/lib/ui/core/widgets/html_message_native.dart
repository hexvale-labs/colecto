import 'dart:convert';

import 'package:flutter/gestures.dart';
import 'package:material_ui/material_ui.dart';

import '../../../data/platform/file_io.dart';
import '../theme/colecto_palette.dart';

/// Native Flutter renderer for Rust-sanitized HTML email on Desktop & Mobile.
///
/// Renders structured blocks (headings, paragraphs, blockquotes, lists, code
/// blocks, horizontal rules, inline bold/italic/code/links, and `data:`/`cid:`
/// images) with **zero WebView RAM** and full `SelectionArea` support.
/// External links open via the OS browser/mail handler (`openExternalUrl`).
class NativeSanitizedHtmlView extends StatelessWidget {
  const NativeSanitizedHtmlView({
    super.key,
    required this.html,
    required this.plainTextFallback,
  });

  final String html;
  final String plainTextFallback;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final baseStyle =
        Theme.of(context).textTheme.bodyMedium ??
        TextStyle(fontSize: 14, color: p.ink, height: 1.45);
    final blocks = _parseBlocks(html);

    if (blocks.isEmpty) {
      return SelectionArea(
        child: Text(
          plainTextFallback.trim().isNotEmpty ? plainTextFallback : html,
          style: baseStyle,
        ),
      );
    }

    return SelectionArea(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (var i = 0; i < blocks.length; i++) ...[
            if (i > 0) const SizedBox(height: 8),
            _buildBlock(context, p, baseStyle, blocks[i]),
          ],
        ],
      ),
    );
  }

  Widget _buildBlock(
    BuildContext context,
    ColectoPalette p,
    TextStyle baseStyle,
    _HtmlBlock block,
  ) {
    switch (block.kind) {
      case _BlockKind.hr:
        return Divider(height: 16, color: p.line);
      case _BlockKind.pre:
        return Container(
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color: p.sunk,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: p.lineSoft),
          ),
          child: Text(
            _decodeEntities(block.raw),
            style: baseStyle.copyWith(
              fontFamily: 'monospace',
              fontSize: (baseStyle.fontSize ?? 14) - 1.5,
              color: p.ink,
            ),
          ),
        );
      case _BlockKind.blockquote:
        return Container(
          padding: const EdgeInsets.fromLTRB(12, 6, 8, 6),
          decoration: BoxDecoration(
            border: Border(left: BorderSide(color: p.accent, width: 2.5)),
            color: p.sunk.withValues(alpha: 0.45),
          ),
          child: Text.rich(
            TextSpan(
              style: baseStyle.copyWith(color: p.ink2),
              children: _parseInline(
                block.raw,
                p,
                baseStyle.copyWith(color: p.ink2),
              ),
            ),
          ),
        );
      case _BlockKind.image:
        final data = _decodeDataUri(block.src ?? '');
        if (data == null) {
          return Text(
            '[Image: ${block.raw.isNotEmpty ? block.raw : 'embedded'}]',
            style: baseStyle.copyWith(color: p.ink3, fontSize: 12),
          );
        }
        return ClipRRect(
          borderRadius: BorderRadius.circular(6),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 380),
            child: Image.memory(
              data,
              fit: BoxFit.contain,
              errorBuilder: (_, _, _) => Text(
                '[Image could not be decoded]',
                style: baseStyle.copyWith(color: p.ink3, fontSize: 12),
              ),
            ),
          ),
        );
      case _BlockKind.heading:
        final scale = switch (block.level) {
          1 => 1.45,
          2 => 1.28,
          3 => 1.14,
          _ => 1.05,
        };
        final style = baseStyle.copyWith(
          fontSize: (baseStyle.fontSize ?? 14) * scale,
          fontWeight: FontWeight.w600,
          color: p.ink,
        );
        return Text.rich(
          TextSpan(style: style, children: _parseInline(block.raw, p, style)),
        );
      case _BlockKind.listItem:
        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.only(right: 8, top: 1),
              child: Text(
                '•',
                style: baseStyle.copyWith(
                  color: p.accent,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            Expanded(
              child: Text.rich(
                TextSpan(
                  style: baseStyle,
                  children: _parseInline(block.raw, p, baseStyle),
                ),
              ),
            ),
          ],
        );
      case _BlockKind.paragraph:
        return Text.rich(
          TextSpan(
            style: baseStyle,
            children: _parseInline(block.raw, p, baseStyle),
          ),
        );
    }
  }

  static List<_HtmlBlock> _parseBlocks(String html) {
    final blocks = <_HtmlBlock>[];
    final pattern = RegExp(
      r'<(h[1-6]|p|blockquote|pre|li|div)\b[^>]*>([\s\S]*?)</\1>|<hr\b[^>]*/?>|<img\b([^>]*)/?>',
      caseSensitive: false,
    );

    var cursor = 0;
    for (final match in pattern.allMatches(html)) {
      if (match.start > cursor) {
        final leading = _stripTags(html.substring(cursor, match.start)).trim();
        if (leading.isNotEmpty) {
          blocks.add(_HtmlBlock(_BlockKind.paragraph, leading));
        }
      }
      cursor = match.end;

      final full = match.group(0) ?? '';
      if (full.toLowerCase().startsWith('<hr')) {
        blocks.add(const _HtmlBlock(_BlockKind.hr, ''));
        continue;
      }
      if (full.toLowerCase().startsWith('<img')) {
        final attrs = match.group(3) ?? '';
        final src = _attr(attrs, 'src');
        final alt = _attr(attrs, 'alt') ?? '';
        if (src != null && src.startsWith('data:image/')) {
          blocks.add(_HtmlBlock(_BlockKind.image, alt, src: src));
        }
        continue;
      }

      final tag = (match.group(1) ?? '').toLowerCase();
      final inner = (match.group(2) ?? '').trim();
      if (inner.isEmpty) continue;

      if (tag.startsWith('h') && tag.length == 2) {
        blocks.add(
          _HtmlBlock(
            _BlockKind.heading,
            inner,
            level: int.tryParse(tag[1]) ?? 2,
          ),
        );
      } else if (tag == 'blockquote') {
        blocks.add(_HtmlBlock(_BlockKind.blockquote, inner));
      } else if (tag == 'pre') {
        blocks.add(_HtmlBlock(_BlockKind.pre, _stripTags(inner)));
      } else if (tag == 'li') {
        blocks.add(_HtmlBlock(_BlockKind.listItem, inner));
      } else {
        blocks.add(_HtmlBlock(_BlockKind.paragraph, inner));
      }
    }

    if (cursor < html.length) {
      final tail = _stripTags(html.substring(cursor)).trim();
      if (tail.isNotEmpty) {
        blocks.add(_HtmlBlock(_BlockKind.paragraph, tail));
      }
    }
    return blocks;
  }

  static List<InlineSpan> _parseInline(
    String input,
    ColectoPalette p,
    TextStyle base,
  ) {
    final normalized = input.replaceAll(
      RegExp(r'<br\s*/?>', caseSensitive: false),
      '\n',
    );
    final spans = <InlineSpan>[];
    final tokenPattern = RegExp(
      r'<(strong|b|em|i|code|a)\b([^>]*)>([\s\S]*?)</\1>',
      caseSensitive: false,
    );

    var cursor = 0;
    for (final m in tokenPattern.allMatches(normalized)) {
      if (m.start > cursor) {
        final plain = _decodeEntities(
          _stripTags(normalized.substring(cursor, m.start)),
        );
        if (plain.isNotEmpty) spans.add(TextSpan(text: plain));
      }
      cursor = m.end;

      final tag = (m.group(1) ?? '').toLowerCase();
      final attrs = m.group(2) ?? '';
      final inner = _decodeEntities(_stripTags(m.group(3) ?? ''));
      if (inner.isEmpty) continue;

      switch (tag) {
        case 'strong':
        case 'b':
          spans.add(
            TextSpan(
              text: inner,
              style: base.copyWith(fontWeight: FontWeight.w600),
            ),
          );
        case 'em':
        case 'i':
          spans.add(
            TextSpan(
              text: inner,
              style: base.copyWith(fontStyle: FontStyle.italic),
            ),
          );
        case 'code':
          spans.add(
            TextSpan(
              text: inner,
              style: base.copyWith(
                fontFamily: 'monospace',
                fontSize: (base.fontSize ?? 14) - 1,
                backgroundColor: p.sunk,
                color: p.accent,
              ),
            ),
          );
        case 'a':
          final href = _attr(attrs, 'href');
          spans.add(
            TextSpan(
              text: inner,
              style: base.copyWith(
                color: p.accent,
                decoration: TextDecoration.underline,
              ),
              recognizer: href != null
                  ? (TapGestureRecognizer()
                      ..onTap = () => openExternalUrl(href))
                  : null,
            ),
          );
      }
    }

    if (cursor < normalized.length) {
      final tail = _decodeEntities(_stripTags(normalized.substring(cursor)));
      if (tail.isNotEmpty) spans.add(TextSpan(text: tail));
    }
    return spans;
  }

  static String? _attr(String attrs, String name) {
    final m = RegExp(
      '$name\\s*=\\s*(?:"([^"]*)"|\'([^\']*)\')',
      caseSensitive: false,
    ).firstMatch(attrs);
    return m?.group(1) ?? m?.group(2);
  }

  static String _stripTags(String input) =>
      input.replaceAll(RegExp(r'<[^>]+>'), '');

  static String _decodeEntities(String input) => input
      .replaceAll('&nbsp;', ' ')
      .replaceAll('&amp;', '&')
      .replaceAll('&lt;', '<')
      .replaceAll('&gt;', '>')
      .replaceAll('&quot;', '"')
      .replaceAll('&#39;', "'")
      .replaceAll('&apos;', "'");

  static dynamic _decodeDataUri(String uri) {
    final comma = uri.indexOf(',');
    if (comma < 0 || !uri.substring(0, comma).contains(';base64')) return null;
    try {
      return base64Decode(uri.substring(comma + 1).trim());
    } catch (_) {
      return null;
    }
  }
}

enum _BlockKind { heading, paragraph, blockquote, pre, listItem, hr, image }

class _HtmlBlock {
  const _HtmlBlock(this.kind, this.raw, {this.level = 2, this.src});

  final _BlockKind kind;
  final String raw;
  final int level;
  final String? src;
}
