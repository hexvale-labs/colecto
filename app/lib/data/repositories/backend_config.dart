import 'package:flutter/foundation.dart';

import '../../ui/features/shell/mail_controller.dart';
import 'session_token_store_stub.dart'
    if (dart.library.js_interop) 'session_token_store_web.dart';

const String _envBackend = String.fromEnvironment('COLECTO_BACKEND');
const String _envDaemonUrl = String.fromEnvironment('COLECTO_DAEMON_URL');
const String _envToken = String.fromEnvironment('COLECTO_TOKEN');

const String defaultDaemonUrl = 'http://127.0.0.1:4173';

class BackendBootstrapConfig {
  const BackendBootstrapConfig({
    required this.kind,
    required this.daemonUrl,
    required this.token,
  });

  final BackendKind kind;
  final String daemonUrl;
  final String token;
}

/// Resolves the initial backend configuration from compile-time `--dart-define`
/// values, Web same-origin detection (when served by `colecto-daemon`), and
/// any token persisted in `sessionStorage` for the current browser tab.
BackendBootstrapConfig resolveBackendConfig({
  String? envBackend,
  String? envDaemonUrl,
  String? envToken,
  bool? isWeb,
  Uri? baseUri,
  String? storedToken,
}) {
  final backendName = (envBackend ?? _envBackend).trim().toLowerCase();
  final definedUrl = (envDaemonUrl ?? _envDaemonUrl).trim();
  final definedToken = (envToken ?? _envToken).trim();
  final webMode = isWeb ?? kIsWeb;
  final uri = baseUri ?? Uri.base;
  final sameOrigin = webMode && (uri.scheme == 'http' || uri.scheme == 'https')
      ? uri.origin
      : '';
  final token = definedToken.isNotEmpty
      ? definedToken
      : (storedToken ?? readSessionToken()).trim();

  final effectiveUrl = definedUrl.isNotEmpty
      ? definedUrl
      : (sameOrigin.isNotEmpty ? sameOrigin : defaultDaemonUrl);

  switch (backendName) {
    case 'demo':
      return BackendBootstrapConfig(
        kind: BackendKind.demo,
        daemonUrl: effectiveUrl,
        token: token,
      );
    case 'remote':
    case 'http':
    case 'daemon':
      return BackendBootstrapConfig(
        kind: BackendKind.remoteHttp,
        daemonUrl: effectiveUrl,
        token: token,
      );
    case 'ffi':
    case 'local':
      return BackendBootstrapConfig(
        kind: BackendKind.localFfi,
        daemonUrl: effectiveUrl,
        token: token,
      );
  }

  // No explicit COLECTO_BACKEND set:
  // - On Web served over http/https (e.g. via `colecto-daemon`), default to
  //   same-origin RemoteHttpBackend so the user can connect immediately or
  //   switch to Demo mode from the connection screen.
  // - On native platforms, default to RemoteHttpBackend if a daemon URL was
  //   provided; otherwise start in Demo mode until `colecto-ffi` is linked.
  if (sameOrigin.isNotEmpty || definedUrl.isNotEmpty) {
    return BackendBootstrapConfig(
      kind: BackendKind.remoteHttp,
      daemonUrl: effectiveUrl,
      token: token,
    );
  }

  return BackendBootstrapConfig(
    kind: BackendKind.demo,
    daemonUrl: defaultDaemonUrl,
    token: token,
  );
}
