import 'package:web/web.dart' as web;

const _kSessionTokenKey = 'colecto.daemon.token';

/// Reads the daemon bearer token stored for the current browser tab session.
String readSessionToken() {
  try {
    return web.window.sessionStorage.getItem(_kSessionTokenKey)?.trim() ?? '';
  } catch (_) {
    return '';
  }
}

/// Persists the daemon bearer token in `sessionStorage` for the active tab.
void writeSessionToken(String token) {
  try {
    final trimmed = token.trim();
    if (trimmed.isEmpty) {
      web.window.sessionStorage.removeItem(_kSessionTokenKey);
    } else {
      web.window.sessionStorage.setItem(_kSessionTokenKey, trimmed);
    }
  } catch (_) {
    // Ignore storage quota or privacy-mode errors.
  }
}

/// Clears any stored daemon bearer token from `sessionStorage`.
void clearSessionToken() {
  try {
    web.window.sessionStorage.removeItem(_kSessionTokenKey);
  } catch (_) {
    // Ignore storage errors.
  }
}
