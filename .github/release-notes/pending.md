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
new extension API 1. The Oneko example is gone until its rewrite.

