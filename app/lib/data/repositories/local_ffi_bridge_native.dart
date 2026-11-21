import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';
import 'dart:isolate';

import 'package:flutter/foundation.dart' show kDebugMode;

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

/// Low-level C-ABI handle used inside worker isolates (never on the UI isolate).
class _FfiSymbols {
  _FfiSymbols._(this.lib)
    : init = lib.lookupFunction<_InitNative, _InitDart>('colecto_ffi_init'),
      request = lib.lookupFunction<_RequestNative, _RequestDart>(
        'colecto_ffi_request',
      ),
      pollEvent = lib.lookupFunction<_PollEventNative, _PollEventDart>(
        'colecto_ffi_poll_event',
      ),
      freeString = lib.lookupFunction<_FreeStringNative, _FreeStringDart>(
        'colecto_ffi_free_string',
      ),
      shutdown = lib.lookupFunction<_ShutdownNative, _ShutdownDart>(
        'colecto_ffi_shutdown',
      );

  final ffi.DynamicLibrary lib;
  final _InitDart init;
  final _RequestDart request;
  final _PollEventDart pollEvent;
  final _FreeStringDart freeString;
  final _ShutdownDart shutdown;

  static _FfiSymbols open({String? libraryPath, required bool debugMode}) {
    final candidates = <String>[
      if (libraryPath != null && libraryPath.isNotEmpty) libraryPath,
      if (Platform.environment['COLECTO_FFI_LIB']?.isNotEmpty == true)
        Platform.environment['COLECTO_FFI_LIB']!,
      if (Platform.isLinux) ...[
        'libcolecto_ffi.so',
        if (debugMode) ...[
          '../target/release/libcolecto_ffi.so',
          '../target/debug/libcolecto_ffi.so',
        ],
      ],
      if (Platform.isMacOS) ...[
        '@executable_path/../Frameworks/libcolecto_ffi.dylib',
        'libcolecto_ffi.dylib',
        if (debugMode) ...[
          '../target/release/libcolecto_ffi.dylib',
          '../target/debug/libcolecto_ffi.dylib',
        ],
      ],
      if (Platform.isWindows) ...[
        'colecto_ffi.dll',
        if (debugMode) ...[
          r'..\target\release\colecto_ffi.dll',
          r'..\target\debug\colecto_ffi.dll',
        ],
      ],
      if (Platform.isAndroid) 'libcolecto_ffi.so',
    ];

    Object? lastError;
    for (final path in candidates) {
      try {
        return _FfiSymbols._(ffi.DynamicLibrary.open(path));
      } catch (e) {
        lastError = e;
      }
    }
    if (Platform.isIOS) {
      return _FfiSymbols._(ffi.DynamicLibrary.process());
    }
    throw StateError(
      'Unable to load libcolecto_ffi native library. Build it with `cargo build -p colecto-ffi` '
      'or set COLECTO_FFI_LIB. ($lastError)',
    );
  }

  String consumeCString(ffi.Pointer<ffi.Char> ptr) {
    if (ptr == ffi.nullptr) return '';
    try {
      final len = _strlen(ptr);
      if (len == 0) return '';
      final bytes = ptr.cast<ffi.Uint8>().asTypedList(len);
      return utf8.decode(bytes, allowMalformed: true);
    } finally {
      freeString(ptr);
    }
  }

  ffi.Pointer<ffi.Char> toCString(String text) {
    final encoded = utf8.encode(text);
    final ptr = _calloc(encoded.length + 1, 1);
    if (ptr == ffi.nullptr) {
      throw StateError('Out of native memory allocating FFI request string.');
    }
    final view = ptr.cast<ffi.Uint8>().asTypedList(encoded.length + 1);
    view.setAll(0, encoded);
    view[encoded.length] = 0;
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

  static final int Function(ffi.Pointer<ffi.Char>) _strlen = _libc
      .lookupFunction<
        ffi.Size Function(ffi.Pointer<ffi.Char>),
        int Function(ffi.Pointer<ffi.Char>)
      >('strlen');
}

/// Long-lived worker isolate that owns the FFI session and processes requests
/// off the Flutter UI thread.
void _requestWorkerEntry(List<Object?> args) {
  final replyPort = args[0]! as SendPort;
  final dataDir = args[1] as String?;
  final libraryPath = args[2] as String?;
  final debugMode = args[3]! as bool;

  final _FfiSymbols syms;
  try {
    syms = _FfiSymbols.open(libraryPath: libraryPath, debugMode: debugMode);
    final dirPtr = dataDir != null ? syms.toCString(dataDir) : ffi.nullptr;
    try {
      final resPtr = syms.init(dirPtr);
      final raw = syms.consumeCString(resPtr);
      final parsed = jsonDecode(raw) as Map<String, dynamic>;
      if (parsed['ok'] != true) {
        throw StateError(
          parsed['error']?.toString() ?? 'Failed to initialize Colecto FFI.',
        );
      }
    } finally {
      if (dirPtr != ffi.nullptr) _FfiSymbols._freeCString(dirPtr);
    }
  } catch (e) {
    replyPort.send(<Object?>['init_error', e.toString()]);
    return;
  }

  final inbox = ReceivePort();
  replyPort.send(<Object?>['ready', inbox.sendPort]);

  inbox.listen((message) {
    if (message is! List<Object?> || message.isEmpty) return;
    final kind = message[0] as String;
    if (kind == 'shutdown') {
      syms.shutdown();
      inbox.close();
      return;
    }
    if (kind == 'request') {
      final reqJson = message[1]! as String;
      final port = message[2]! as SendPort;
      final reqPtr = syms.toCString(reqJson);
      try {
        final respPtr = syms.request(reqPtr);
        final raw = syms.consumeCString(respPtr);
        port.send(<Object?>['ok', raw]);
      } catch (e) {
        port.send(<Object?>['err', e.toString()]);
      } finally {
        _FfiSymbols._freeCString(reqPtr);
      }
    }
  });
}

/// Dedicated event-polling isolate that waits on Rust's `colecto_ffi_poll_event`
/// without blocking either the UI isolate or the request worker isolate.
void _eventWorkerEntry(List<Object?> args) {
  final outPort = args[0]! as SendPort;
  final libraryPath = args[1] as String?;
  final debugMode = args[2]! as bool;

  final _FfiSymbols syms;
  try {
    syms = _FfiSymbols.open(libraryPath: libraryPath, debugMode: debugMode);
  } catch (_) {
    return;
  }

  final control = ReceivePort();
  outPort.send(<Object?>['control', control.sendPort]);
  var stopped = false;
  control.listen((msg) {
    if (msg == 'stop') {
      stopped = true;
      control.close();
    }
  });

  // Yield to the control port between blocking 250 ms polls, draining any
  // immediately queued events with timeout 0 before waiting again.
  void loop() {
    if (stopped) return;
    var ptr = syms.pollEvent(250);
    while (ptr != ffi.nullptr && !stopped) {
      final raw = syms.consumeCString(ptr);
      if (raw.isNotEmpty) {
        outPort.send(<Object?>['event', raw]);
      }
      ptr = syms.pollEvent(0);
    }
    if (!stopped) {
      Timer.run(loop);
    }
  }

  Timer.run(loop);
}

/// Native `dart:ffi` bridge to `libcolecto_ffi` (`crates/colecto-ffi`).
///
/// Communicates directly with the in-process Rust `MailService` with zero
/// loopback TCP ports open, running all FFI calls on a background worker
/// isolate so the Flutter UI thread never blocks.
class NativeFfiBridge {
  NativeFfiBridge({this.dataDir, this.libraryPath});

  final String? dataDir;
  final String? libraryPath;

  Future<SendPort>? _workerPortFuture;
  SendPort? _workerPort;
  Isolate? _workerIsolate;
  bool _disposed = false;

  Future<SendPort> _ensureWorker() {
    if (_disposed) {
      return Future<SendPort>.error(
        StateError('Colecto FFI bridge has been disposed.'),
      );
    }
    return _workerPortFuture ??= _spawnWorker();
  }

  Future<SendPort> _spawnWorker() async {
    final initPort = ReceivePort();
    try {
      final isolate = await Isolate.spawn(_requestWorkerEntry, <Object?>[
        initPort.sendPort,
        dataDir,
        libraryPath,
        kDebugMode,
      ], debugName: 'colecto-ffi-worker');
      final msg = await initPort.first as List<Object?>;
      if (msg[0] == 'init_error') {
        isolate.kill(priority: Isolate.immediate);
        _workerPortFuture = null;
        throw StateError(msg[1]! as String);
      }
      _workerIsolate = isolate;
      _workerPort = msg[1]! as SendPort;
      return _workerPort!;
    } catch (e) {
      _workerPortFuture = null;
      rethrow;
    } finally {
      initPort.close();
    }
  }

  Future<FfiResponse> request({
    required String method,
    required String path,
    String? account,
    String? body,
  }) async {
    final worker = await _ensureWorker();
    final reqJson = jsonEncode({
      'method': method,
      'path': path,
      if (account != null && account.isNotEmpty) 'account': account,
      'body': ?body,
    });
    final reply = ReceivePort();
    try {
      worker.send(<Object?>['request', reqJson, reply.sendPort]);
      final res = await reply.first as List<Object?>;
      if (res[0] == 'err') {
        throw StateError(res[1]! as String);
      }
      final raw = res[1]! as String;
      final map = jsonDecode(raw) as Map<String, dynamic>;
      return FfiResponse(
        status: (map['status'] as num?)?.toInt() ?? 500,
        contentType: map['content_type'] as String? ?? 'application/json',
        contentDisposition: map['content_disposition'] as String?,
        body: map['body'] as String? ?? '',
        isBase64: map['is_base64'] == true,
      );
    } finally {
      reply.close();
    }
  }

  Stream<ServiceEvent> events() {
    late StreamController<ServiceEvent> controller;
    ReceivePort? port;
    SendPort? stopPort;
    Isolate? eventIsolate;
    var cancelled = false;

    Future<void> start() async {
      try {
        await _ensureWorker();
      } catch (_) {
        return;
      }
      if (cancelled || _disposed || controller.isClosed) return;
      final rx = ReceivePort();
      port = rx;
      rx.listen((msg) {
        if (msg is! List<Object?> || msg.length < 2) return;
        if (msg[0] == 'control') {
          stopPort = msg[1] as SendPort?;
          if (cancelled || _disposed) stopPort?.send('stop');
        } else if (msg[0] == 'event' && !controller.isClosed) {
          final parsed = _parseEvent(msg[1]! as String);
          if (parsed != null) controller.add(parsed);
        }
      });
      eventIsolate = await Isolate.spawn(_eventWorkerEntry, <Object?>[
        rx.sendPort,
        libraryPath,
        kDebugMode,
      ], debugName: 'colecto-ffi-events');
    }

    controller = StreamController<ServiceEvent>.broadcast(
      onListen: () {
        cancelled = false;
        unawaited(start());
      },
      onCancel: () {
        cancelled = true;
        stopPort?.send('stop');
        port?.close();
        port = null;
        eventIsolate?.kill(priority: Isolate.immediate);
        eventIsolate = null;
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

  void dispose() {
    _disposed = true;
    final port = _workerPort;
    _workerPort = null;
    _workerPortFuture = null;
    if (port != null) {
      port.send(const <Object?>['shutdown']);
    } else {
      _workerIsolate?.kill(priority: Isolate.immediate);
    }
    _workerIsolate = null;
  }
}
