import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';

import '../../domain/models/account_models.dart';
import '../../domain/models/service_event.dart';
import 'local_ffi_bridge_stub.dart' show FfiResponse;

typedef _InitNative = ffi.Pointer<ffi.Char> Function(ffi.Pointer<ffi.Char>);
typedef _InitDart = ffi.Pointer<ffi.Char> Function(ffi.Pointer<ffi.Char>);

typedef _RequestNative = ffi.Pointer<ffi.Char> Function(ffi.Pointer<ffi.Char>);
typedef _RequestDart = ffi.Pointer<ffi.Char> Function(ffi.Pointer<ffi.Char>);

typedef _PollEventNative = ffi.Pointer<ffi.Char> Function(ffi.Uint64);
typedef _PollEventDart = ffi.Pointer<ffi.Char> Function(int);

typedef _FreeStringNative = ffi.Void Function(ffi.Pointer<ffi.Char>);
typedef _FreeStringDart = void Function(ffi.Pointer<ffi.Char>);

typedef _ShutdownNative = ffi.Void Function();
typedef _ShutdownDart = void Function();

/// Native `dart:ffi` bridge to `libcolecto_ffi` (`crates/colecto-ffi`).
///
/// Communicates directly with the in-process Rust `MailService` with zero
/// loopback TCP ports open.
class NativeFfiBridge {
  NativeFfiBridge({this.dataDir, this.libraryPath});

  final String? dataDir;
  final String? libraryPath;

  ffi.DynamicLibrary? _lib;
  _InitDart? _init;
  _RequestDart? _request;
  _PollEventDart? _pollEvent;
  _FreeStringDart? _freeString;
  _ShutdownDart? _shutdown;
  bool _initialized = false;
  bool _disposed = false;

  bool get isAvailable {
    try {
      _loadLibrary();
      return true;
    } catch (_) {
      return false;
    }
  }

  ffi.DynamicLibrary _loadLibrary() {
    if (_lib != null) return _lib!;
    final candidates = <String>[
      if (libraryPath != null && libraryPath!.isNotEmpty) libraryPath!,
      if (Platform.environment['COLECTO_FFI_LIB']?.isNotEmpty == true)
        Platform.environment['COLECTO_FFI_LIB']!,
      if (Platform.isLinux) ...[
        'libcolecto_ffi.so',
        '../target/release/libcolecto_ffi.so',
        '../target/debug/libcolecto_ffi.so',
      ],
      if (Platform.isMacOS) ...[
        'libcolecto_ffi.dylib',
        '../target/release/libcolecto_ffi.dylib',
        '../target/debug/libcolecto_ffi.dylib',
      ],
      if (Platform.isWindows) ...[
        'colecto_ffi.dll',
        r'..\target\release\colecto_ffi.dll',
        r'..\target\debug\colecto_ffi.dll',
      ],
      if (Platform.isAndroid) 'libcolecto_ffi.so',
    ];

    Object? lastError;
    for (final path in candidates) {
      try {
        final lib = ffi.DynamicLibrary.open(path);
        _lib = lib;
        _init = lib.lookupFunction<_InitNative, _InitDart>('colecto_ffi_init');
        _request = lib.lookupFunction<_RequestNative, _RequestDart>(
          'colecto_ffi_request',
        );
        _pollEvent = lib.lookupFunction<_PollEventNative, _PollEventDart>(
          'colecto_ffi_poll_event',
        );
        _freeString = lib.lookupFunction<_FreeStringNative, _FreeStringDart>(
          'colecto_ffi_free_string',
        );
        _shutdown = lib.lookupFunction<_ShutdownNative, _ShutdownDart>(
          'colecto_ffi_shutdown',
        );
        return lib;
      } catch (e) {
        lastError = e;
      }
    }
    if (Platform.isIOS) {
      final lib = ffi.DynamicLibrary.process();
      _lib = lib;
      _init = lib.lookupFunction<_InitNative, _InitDart>('colecto_ffi_init');
      _request = lib.lookupFunction<_RequestNative, _RequestDart>(
        'colecto_ffi_request',
      );
      _pollEvent = lib.lookupFunction<_PollEventNative, _PollEventDart>(
        'colecto_ffi_poll_event',
      );
      _freeString = lib.lookupFunction<_FreeStringNative, _FreeStringDart>(
        'colecto_ffi_free_string',
      );
      _shutdown = lib.lookupFunction<_ShutdownNative, _ShutdownDart>(
        'colecto_ffi_shutdown',
      );
      return lib;
    }
    throw StateError(
      'Unable to load libcolecto_ffi native library. Build it with `cargo build -p colecto-ffi` '
      'or set COLECTO_FFI_LIB. ($lastError)',
    );
  }

  void ensureInitialized() {
    if (_initialized) return;
    _loadLibrary();
    final dirPtr = dataDir != null ? _toCString(dataDir!) : ffi.nullptr;
    try {
      final resPtr = _init!(dirPtr);
      final raw = _consumeCString(resPtr);
      final parsed = jsonDecode(raw) as Map<String, dynamic>;
      if (parsed['ok'] != true) {
        throw StateError(
          parsed['error']?.toString() ?? 'Failed to initialize Colecto FFI.',
        );
      }
      _initialized = true;
    } finally {
      if (dirPtr != ffi.nullptr) _freeCString(dirPtr);
    }
  }

  FfiResponse request({
    required String method,
    required String path,
    String? account,
    String? body,
  }) {
    ensureInitialized();
    final reqJson = jsonEncode({
      'method': method,
      'path': path,
      if (account != null && account.isNotEmpty) 'account': account,
      'body': ?body,
    });
    final reqPtr = _toCString(reqJson);
    try {
      final respPtr = _request!(reqPtr);
      final raw = _consumeCString(respPtr);
      final map = jsonDecode(raw) as Map<String, dynamic>;
      return FfiResponse(
        status: (map['status'] as num?)?.toInt() ?? 500,
        contentType: map['content_type'] as String? ?? 'application/json',
        contentDisposition: map['content_disposition'] as String?,
        body: map['body'] as String? ?? '',
        isBase64: map['is_base64'] == true,
      );
    } finally {
      _freeCString(reqPtr);
    }
  }

  Stream<ServiceEvent> events() {
    late StreamController<ServiceEvent> controller;
    Timer? timer;

    controller = StreamController<ServiceEvent>.broadcast(
      onListen: () {
        if (!_initialized) return;
        timer = Timer.periodic(const Duration(milliseconds: 250), (_) {
          if (_disposed || controller.isClosed || _pollEvent == null) return;
          final ptr = _pollEvent!(0);
          if (ptr == ffi.nullptr) return;
          final raw = _consumeCString(ptr);
          final parsed = _parseEvent(raw);
          if (parsed != null && !controller.isClosed) {
            controller.add(parsed);
          }
        });
      },
      onCancel: () {
        timer?.cancel();
      },
    );
    return controller.stream;
  }

  ServiceEvent? _parseEvent(String raw) {
    try {
      final env = jsonDecode(raw) as Map<String, dynamic>;
      final name = env['event'] as String? ?? '';
      final payload =
          (env['data'] as Map<String, dynamic>?) ?? const <String, dynamic>{};
      return switch (name) {
        'ready' => const ReadyServiceEvent(),
        'resync' => const ResyncServiceEvent(),
        'mail' => MailServiceEvent(
          account: payload['account'] as String? ?? '',
          ids: (payload['ids'] as List<dynamic>? ?? const [])
              .map((e) => e.toString())
              .toList(growable: false),
        ),
        'sync' => SyncServiceEvent(AccountSync.fromJson(payload)),
        'approval' => ApprovalServiceEvent(
          id: payload['id'] as String? ?? '',
          account: payload['account'] as String? ?? '',
          from: payload['from'] as String? ?? '',
          subject: payload['subject'] as String? ?? '',
        ),
        'navigate' => NavigateServiceEvent(
          target: payload['target'] as String? ?? '',
          account: payload['account'] as String? ?? '',
        ),
        'mcp' => McpServiceEvent(on: payload['on'] == true),
        'preferences' => const PreferencesServiceEvent(),
        'labels' => LabelsUpdatedServiceEvent(
          account: payload['account'] as String? ?? '',
        ),
        _ => null,
      };
    } catch (_) {
      return null;
    }
  }

  String _consumeCString(ffi.Pointer<ffi.Char> ptr) {
    if (ptr == ffi.nullptr) return '';
    try {
      final bytes = <int>[];
      var offset = 0;
      while (true) {
        final byte = (ptr + offset).value;
        if (byte == 0) break;
        bytes.add(byte & 0xFF);
        offset++;
      }
      return utf8.decode(bytes, allowMalformed: true);
    } finally {
      _freeString?.call(ptr);
    }
  }

  ffi.Pointer<ffi.Char> _toCString(String text) {
    final encoded = utf8.encode(text);
    final ptr = _calloc(encoded.length + 1, 1);
    for (var i = 0; i < encoded.length; i++) {
      (ptr + i).value = encoded[i];
    }
    (ptr + encoded.length).value = 0;
    return ptr;
  }

  static final ffi.DynamicLibrary _libc = Platform.isWindows
      ? ffi.DynamicLibrary.open('msvcrt.dll')
      : ffi.DynamicLibrary.process();

  static final ffi.Pointer<ffi.Char> Function(int, int) _calloc = _libc
      .lookupFunction<
        ffi.Pointer<ffi.Char> Function(ffi.Size, ffi.Size),
        ffi.Pointer<ffi.Char> Function(int, int)
      >('calloc');

  static final void Function(ffi.Pointer<ffi.Char>) _freeCString = _libc
      .lookupFunction<
        ffi.Void Function(ffi.Pointer<ffi.Char>),
        void Function(ffi.Pointer<ffi.Char>)
      >('free');

  void dispose() {
    _disposed = true;
    if (_initialized) {
      _shutdown?.call();
      _initialized = false;
    }
  }
}
