import 'dart:async';

import '../../domain/models/service_event.dart';

class FfiResponse {
  const FfiResponse({
    required this.status,
    required this.contentType,
    required this.body,
    required this.isBase64,
    this.contentDisposition,
  });

  final int status;
  final String contentType;
  final String? contentDisposition;
  final String body;
  final bool isBase64;
}

/// Web/stub implementation of the in-process Rust FFI bridge.
class NativeFfiBridge {
  NativeFfiBridge({this.dataDir, this.libraryPath});

  final String? dataDir;
  final String? libraryPath;

  bool get isAvailable => false;

  void ensureInitialized() {
    throw UnsupportedError(
      'In-process Rust FFI is unavailable on Web. Use Remote Daemon or Demo mode.',
    );
  }

  Future<FfiResponse> request({
    required String method,
    required String path,
    String? account,
    String? body,
  }) async {
    ensureInitialized();
    throw UnimplementedError();
  }

  Stream<ServiceEvent> events() => const Stream<ServiceEvent>.empty();

  void dispose() {}
}
