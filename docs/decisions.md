# Architectural Decisions

Record of the clean-room architecture decisions behind the Flutter + Rust Colecto rewrite.

## 1. Clean-room Flutter frontend, reused Rust backend
- **Decision:** Rewrite the frontend in Flutter while reusing and simplifying the proven Rust mail engine (`colecto-core`, `colecto-service`, `colecto-daemon`, and `colecto-ffi`).
- **Why:** `colecto`'s Rust backend already solves the hardest mail problems (IMAP/SMTP, JMAP, MIME, Turso embedded storage, XChaCha20-Poly1305 vault, autodiscovery, IDLE/push, AI, MCP). Rewriting those in Dart would lose embedded Turso, degrade JMAP/discovery/daemon support, add GC churn on large MIME payloads, and re-introduce protocol bugs.

## 2. Drop the extension system completely
- **Decision:** Zero extension API, zero Wasm UI sandbox, zero Web Worker component protocol.
- **Why:** In `colecto`, maintaining a versioned sandboxed extension system consumed 5 of 9 Rust crates plus a custom UI runtime and renderer. With modern AI-assisted development, the barrier to opening a PR for a first-party feature is tiny; a clean, modular first-party codebase is vastly easier to maintain and contribute to.

## 3. Flutter + `flutter_rust_bridge` over `nsh.dev` and Tauri
- **Decision:** Use Flutter (compiled natively on Android/iOS/Linux/Windows/macOS and to WebAssembly for Web) bridged to Rust via `flutter_rust_bridge` (local) and HTTP/SSE (Web / remote daemon).
- **Why not Tauri / WebView UI:** Mobile UX felt bolted on, and hosting the entire UI in a WebView sets a high baseline RAM floor.
- **Why not `nsh.dev` (for now):** `nsh.dev` (Longbridge Native Shell) has an exciting Rust-native architecture, but is still very young—meaning high risk of debugging six native backends instead of building mail, missing mobile OS glue (UnifiedPush, WorkManager, Keystore/Keychain, HTML WebView-in-scroll), and low LLM/agent familiarity that conflicts with the goal of easy agent-authored PRs.
- **Why Flutter works well here:**
  - True 120Hz mobile feel with Material 3 (Android) and Cupertino (iOS).
  - Non-Material, dense shadcn-style UI on Desktop and Web Wasm.
  - Low RAM (Skia/Impeller, no browser engine for app chrome).
  - Flutter Wasm is well-suited for a self-hosted webmail client (no SEO needed, cached after first load).
  - High AI-agent fluency for fast, low-friction PRs.

## 4. Required Initial Spikes (Validate Before Full Buildout)

Before building all screens, validate these two technical risks first:

1. **Linux (Wayland) HTML Email WebView Spike:**
   - Flutter's official `webview_flutter` does not support Linux.
   - Verify an embedded sandboxed HTML email renderer on Linux/Wayland (and Android/Web) inside a scrollable conversation view, supporting dynamic content height, remote-image blocking, and external link interception.
   - *Interim status:* Web renders sanitized HTML inside a real `<iframe sandbox>` (`html_message_iframe_web.dart`), while Desktop and Mobile currently render Rust-sanitized HTML natively via `NativeSanitizedHtmlView` (zero WebView RAM) until a pooled cross-platform WebView is wired in.
2. **`flutter_rust_bridge` + `colecto-core` Skeleton Spike:**
   - Wire `colecto-service` into `colecto-ffi` (account list + folder/message fetch + event stream), generate Dart bindings with `frb_codegen`, and verify builds on Linux desktop, Android, and Flutter Web (talking to `colecto-daemon` over HTTP/SSE).

## 5. Single-File `colecto.db` Storage, No DB-at-Rest Email Encryption, and Planned PGP/S-MIME

- **Single database file (`colecto.db`):**
  - All local state lives inside a single Turso/SQLite database (`colecto.db`, default data directory `.colecto`) tagged with `PRAGMA application_id = 0x434F_4C45` (`"COLE"`) and versioned via forward-only `PRAGMA user_version` migrations (`crates/colecto-core/migrations/`).
  - Never edit a shipped migration; every future schema change adds the next numbered migration. Before applying a newer schema version to an existing database, `colecto-core` creates a pre-migration backup under `backups/v<N>-<timestamp>.db` (retaining the 5 newest backups) and rolls back on failure.
  - Owner and AI settings live in the sparse `settings(scope, key, value)` table (only changed keys are stored; defaults live in code). Structured entities (`drafts`, `labels`, `label_messages`, `api_keys`, `send_approvals`, `ai_summaries`, `ai_autolabel_seen`, mailbox caches) live in proper relational tables; `records` is reserved for atomic opaque documents such as `("core", "vault")`.
  - **No sidecar config or token files:** Never write `colecto-settings.json`, `preferences.json`, `daemon.json`, or `owner-token` files into the data directory.
- **No database-at-rest email encryption:**
  - Cached email envelopes, bodies, drafts, and labels are stored unencrypted in `colecto.db` (relying on OS filesystem/disk encryption and `0700` directory permissions).
  - Account passwords, JMAP tokens, and AI API keys remain sealed in the XChaCha20-Poly1305 credential vault (`("core", "vault")`) backed by the OS keychain or an Argon2id master password.
- **Planned end-to-end email encryption (PGP & S/MIME):**
  - End-to-end message security (OpenPGP signing/encryption/decryption and S/MIME) is planned as a first-party Rust module in `colecto-core` (not database-at-rest encryption).

## 6. One native design system per platform, behind the Colecto kit

- **Decision:** Linux and Web use `shadcn_ui` themed with Colecto's palette; Windows uses `fluent_ui`; macOS uses `macos_ui`; Android uses Material 3 (`material_ui`) with `dynamic_color`; iOS uses `cupertino_ui`. Screens use only the Colecto kit (`app/lib/ui/core/kit/`), which maps each component onto the active design.
- **Why these libraries (researched November 2026):**
  - `fluent_ui`: Flutter Favorite, actively released, full WinUI control set.
  - `macos_ui`: Flutter Favorite with real AppKit-style controls; `cupertino_ui` alone would look like an iPad app on the Mac. `cupertino_native_better` (real AppKit/UIKit views) was rejected for now: one maintainer, very frequent releases, and a platform view per control.
  - Linux has no maintained native toolkit binding: `libadwaita`/`adwaita` have not shipped since 2023 and `yaru` is Material-based. `shadcn_ui` is the most active shadcn-style library and matches Colecto's look.
  - `material_ui` now includes Material 3 Expressive, so no third-party Expressive package is needed; `dynamic_color` provides Material You.
- **Defaults, not limits:** the table is each platform's default. Owners may choose any design on any platform (`Preferences.design`), so every design must work at every window size.
- **Consequence:** adding a control means adding (or extending) a kit component with a mapping for every design. `COLECTO_DESIGN` and Settings → Appearance → Design preview any design on any platform.
