<!--
Release-note text for changes merged into develop but not yet released, mainly
extension API versions (see docs/extensions/api-versions/README.md). Whoever ships the
next release moves this text into .github/release-notes/vX.Y.Z.md and empties this
file back to this comment. Never publish this file itself.
-->

### Extensions rebuilt

The extension system was rebuilt: extensions now run their interface in a sandboxed Web
Worker on your device and describe it with components Colecto draws, with optional
background work in a WebAssembly sandbox. Extensions built for the old system no longer
load and are removed on startup (their settings too); reinstall versions built for the
new extension API 1.

What you may notice:

- Mail, Reader, Composer, Notifications and AI are extensions on the new system. The
  composer has its own tab next to Reader.
- List density and date headings moved to Settings → Mail; toast position and desktop
  notifications moved to Settings → Appearance. Mail, Privacy and Notifications
  settings start from their defaults once.
- Toasts, extension dialogs and extension settings pages name the extension they come
  from.
- AI auto-labels run in the background inside the AI extension's sandbox, even with no
  window open.
- The Oneko example is back, drawing on the new canvas overlay.

