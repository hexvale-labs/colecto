import 'dart:async';
import 'dart:js_interop';
import 'dart:typed_data';

import 'package:web/web.dart' as web;

import 'file_io.dart';

Future<String> saveDownloadedFile({
  required String name,
  required String contentType,
  required Uint8List bytes,
}) async {
  final blob = web.Blob(
    <JSAny>[bytes.toJS].toJS,
    web.BlobPropertyBag(type: contentType),
  );
  final objectUrl = web.URL.createObjectURL(blob);
  final anchor = web.HTMLAnchorElement()
    ..href = objectUrl
    ..download = name
    ..style.display = 'none';
  web.document.body?.append(anchor);
  anchor.click();
  anchor.remove();
  Future<void>.delayed(const Duration(seconds: 5), () {
    web.URL.revokeObjectURL(objectUrl);
  });
  return name;
}

Future<List<PickedAttachmentFile>?> pickAttachmentFiles() async {
  final completer = Completer<List<PickedAttachmentFile>>();
  final input = web.HTMLInputElement()
    ..type = 'file'
    ..multiple = true
    ..style.display = 'none';

  void finish(List<PickedAttachmentFile> files) {
    input.remove();
    if (!completer.isCompleted) completer.complete(files);
  }

  input.addEventListener(
    'change',
    ((web.Event _) {
      unawaited(() async {
        final list = input.files;
        if (list == null || list.length == 0) {
          finish(const []);
          return;
        }
        final picked = <PickedAttachmentFile>[];
        for (var i = 0; i < list.length; i++) {
          final file = list.item(i);
          if (file == null) continue;
          final buffer = await file.arrayBuffer().toDart;
          picked.add(
            PickedAttachmentFile(
              name: file.name,
              contentType: file.type.isNotEmpty
                  ? file.type
                  : 'application/octet-stream',
              bytes: buffer.toDart.asUint8List(),
            ),
          );
        }
        finish(picked);
      }());
    }).toJS,
  );

  input.addEventListener('cancel', ((web.Event _) => finish(const [])).toJS);

  web.document.body?.append(input);
  input.click();
  return completer.future;
}

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
