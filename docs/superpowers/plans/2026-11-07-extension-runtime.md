# Isolated extension runtime implementation plan

Goal: install TOML + Wasm packages, execute them with bounded resources, persist
consented account grants, and supply a Tauri desktop entrypoint.

Architecture: the Host owns package identity and invokes Wasm with no WASI.
Only a typed request import reaches account-scoped service APIs. Guests emit
bounded declarative text/actions, never HTML, JavaScript or native commands.
Bundled Svelte packages remain trusted build-time components. Host controls own
installation, grant consent, recovery and credential entry.

- [x] Persist a versioned policy/package file atomically; validate on load and
      reject corrupt/newer files. Do not modify mail or vault formats.
- [x] Add installation/removal and account-bound execution endpoints, with no
      implicit third-party grants and no replacement of bundled IDs.
- [x] Add Wasm fuel, memory, table, module/input/output and concurrency limits;
      enforce each service request against current grants and Host-bound identity.
- [x] Render guest views as escaped text and actions in Host UI; add file upload,
      metadata/consent controls and error recovery. Never execute guest markup.
- [x] Add optional Tauri desktop runner with no guest IPC or platform permissions.
- [x] Exercise hostile modules, restart persistence, revoked grants, installation,
      rendering and recovery. Run just verify, browser checks and native check.

Compatibility: new extensions.json is Host-owned; no existing mail/account data
moves. Older binaries ignore it. Do not downgrade after revoking access unless
understanding that older binaries lack this enforcement. No mobile parity claim.


Validation: `just verify`, `just ui-test` (11 browser flows plus process restart),
`just native-check`, and `just native-test` (Linux/X11 window and registry smoke).
Independent review found reusable-ID grant inheritance and missing directory
synchronization; both were corrected. The generation regression exercises
removal/reinstallation with the same ID. Native smoke is not a full native UI
automation suite, and Windows/macOS/mobile remain untested.
