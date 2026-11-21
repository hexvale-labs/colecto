import 'dart:io';
import 'dart:typed_data';

import 'file_io.dart';

String _sanitizeFilename(String raw) {
  final base = raw.split(RegExp(r'[/\\]')).last.trim();
  final clean = base.replaceAll(RegExp(r'[^\w.\- ()+]'), '_');
  return clean.isEmpty ? 'attachment' : clean;
}

String _guessContentType(String name) {
  final lower = name.toLowerCase();
  if (lower.endsWith('.pdf')) return 'application/pdf';
  if (lower.endsWith('.png')) return 'image/png';
  if (lower.endsWith('.jpg') || lower.endsWith('.jpeg')) return 'image/jpeg';
  if (lower.endsWith('.gif')) return 'image/gif';
  if (lower.endsWith('.webp')) return 'image/webp';
  if (lower.endsWith('.svg')) return 'image/svg+xml';
  if (lower.endsWith('.txt') || lower.endsWith('.md')) return 'text/plain';
  if (lower.endsWith('.html') || lower.endsWith('.htm')) return 'text/html';
  if (lower.endsWith('.csv')) return 'text/csv';
  if (lower.endsWith('.json')) return 'application/json';
  if (lower.endsWith('.zip')) return 'application/zip';
  return 'application/octet-stream';
}

Directory _downloadsDir() {
  final home =
      Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
  if (home != null && home.isNotEmpty) {
    final downloads = Directory('$home${Platform.pathSeparator}Downloads');
    if (downloads.existsSync()) return downloads;
  }
  return Directory.systemTemp;
}

Future<String> saveDownloadedFile({
  required String name,
  required String contentType,
  required Uint8List bytes,
}) async {
  final dir = _downloadsDir();
  final safe = _sanitizeFilename(name);
  var candidate = File('${dir.path}${Platform.pathSeparator}$safe');
  var suffix = 1;
  while (candidate.existsSync()) {
    final dot = safe.lastIndexOf('.');
    final nextName = dot > 0
        ? '${safe.substring(0, dot)} ($suffix)${safe.substring(dot)}'
        : '$safe ($suffix)';
    candidate = File('${dir.path}${Platform.pathSeparator}$nextName');
    suffix++;
  }
  await candidate.writeAsBytes(bytes, flush: true);
  return candidate.path;
}

Future<List<PickedAttachmentFile>?> pickAttachmentFiles() async {
  try {
    List<String> paths = const [];
    if (Platform.isLinux) {
      final result = await Process.run('zenity', const [
        '--file-selection',
        '--multiple',
        '--separator=\n',
        '--title=Attach files',
      ]);
      if (result.exitCode == 1) return const [];
      if (result.exitCode != 0) return null;
      paths = (result.stdout as String)
          .split('\n')
          .map((e) => e.trim())
          .where((e) => e.isNotEmpty)
          .toList(growable: false);
    } else if (Platform.isMacOS) {
      final result = await Process.run('osascript', const [
        '-e',
        'set chosen to choose file with multiple selections allowed\n'
            'set out to ""\n'
            'repeat with f in chosen\n'
            '  set out to out & POSIX path of f & linefeed\n'
            'end repeat\n'
            'return out',
      ]);
      if (result.exitCode != 0) return const [];
      paths = (result.stdout as String)
          .split('\n')
          .map((e) => e.trim())
          .where((e) => e.isNotEmpty)
          .toList(growable: false);
    } else if (Platform.isWindows) {
      final result = await Process.run('powershell', const [
        '-NoProfile',
        '-Command',
        'Add-Type -AssemblyName System.Windows.Forms; '
            r'$d = New-Object System.Windows.Forms.OpenFileDialog; '
            r'$d.Multiselect = $true; '
            r'if ($d.ShowDialog() -eq "OK") { $d.FileNames -join "`n" }',
      ]);
      if (result.exitCode != 0) return const [];
      paths = (result.stdout as String)
          .split(RegExp(r'\r?\n'))
          .map((e) => e.trim())
          .where((e) => e.isNotEmpty)
          .toList(growable: false);
    } else {
      return null;
    }

    final files = <PickedAttachmentFile>[];
    for (final path in paths) {
      final file = File(path);
      if (!file.existsSync()) continue;
      final bytes = await file.readAsBytes();
      final name = path.split(RegExp(r'[/\\]')).last;
      files.add(
        PickedAttachmentFile(
          name: name,
          contentType: _guessContentType(name),
          bytes: bytes,
        ),
      );
    }
    return files;
  } catch (_) {
    return null;
  }
}

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
      await Process.start('cmd', ['/c', 'start', '', uri.toString()]);
      return true;
    }
  } catch (_) {}
  return false;
}
