# Agent instructions

Colecto is built by AI agents with one human maintainer. These rules apply to every agent. `CLAUDE.md` is a symlink to this file.

## Before you start

- **Identify yourself first:** Before doing anything else in a session, state in 1–2 sentences what model you are, what harness/tool you are running in, and who made you, to the best of your capability (e.g., *"I'm Gemini 4 Neon, running in T3 Code through Antigravity. I am made by Google."*). This ensures downstream text-generation models (such as PR or issue writers) see which model actually did the work and credit everyone accurately.
- Read [README.md](README.md), [docs/architecture.md](docs/architecture.md), [docs/ui-design.md](docs/ui-design.md), and [docs/decisions.md](docs/decisions.md).
- List the skills available in `.agents/skills/` (pinned in `skills-lock.json`; `.claude/skills` symlinks to the same folder) and read the `SKILL.md` of each skill that fits the task before editing.
- For UI work, read [docs/ui-design.md](docs/ui-design.md) and Colecto's `DESIGN.md` principles it carries forward, then follow the Flutter skills (`flutter-*`, `dart-*`) and design skills (`emil-design-eng`, `apple-design`, `animate`, `break-ui`).

## Non-negotiable architectural rules

1. **No extensions or plugin runtimes.** Do not introduce a plugin ABI, Wasm UI sandbox, or dynamic extension loader. Build features cleanly as first-party, toggleable modules.
2. **Rust owns mail, storage, crypto, and sync.** Keep IMAP/SMTP, JMAP, MIME decoding, HTML sanitization, Turso storage, credential vault, autodiscovery, IDLE/push sync, AI, and MCP in Rust. Keep Dart/Flutter focused on presentation, navigation, state, and OS UI glue.
3. **One backend interface in Flutter.** All UI code talks to a single Dart `MailBackend` repository interface with two implementations:
   - `LocalFfiBackend` (`flutter_rust_bridge`) for desktop and mobile standalone mode.
   - `RemoteHttpBackend` (HTTP + SSE) for Flutter Web (Wasm) and remote-daemon mode.
   Never open a loopback HTTP port on desktop or mobile for local mail access.
4. **Each platform gets its own native design system; never Material on desktop or Web.**
   - **Linux & Web (Wasm):** `shadcn_ui`, themed with Colecto's palette (dense, 1px borders, hover states, right-click menus, sharp focus rings, no ripples).
   - **Windows:** `fluent_ui`.
   - **macOS:** `macos_ui`.
   - **Android:** Material 3 (`material_ui`) with `dynamic_color` (Material You on by default; the accent setting overrides it).
   - **iOS:** Cupertino (`cupertino_ui`).
   Screens use only the Colecto kit (`app/lib/ui/core/kit/`: `ColectoButton`, `ColectoText`, `ColectoTextField`, `ColectoSwitch`, menus, dialogs…), which maps each component onto the active design. Never import a design library in feature code; add or extend a kit component instead. Share business logic, controllers, and core mail views; branch only in the kit and the navigation shell. Those are defaults, not limits: any design must work on any platform and at every window size, and owners pick theirs in Settings → Appearance → Design (`Preferences.design`). Preview with `just run demo linux <design>` (`colecto`, `fluent`, `macos`, `material`, `cupertino`).
5. **Untrusted HTML email safety & RAM discipline.**
   - Sanitize HTML in Rust (strip scripts/forms, block remote images by default, rewrite `cid:` inline images).
   - Render plain-text and simple messages natively in Flutter.
   - Render HTML emails in a lazy/pooled sandboxed WebView (or `<iframe sandbox>` via `HtmlElementView` on Web); never keep a WebView alive when no HTML message is open.
6. **Never support POP3.**

## Releases

Ship a release only when the maintainer asks for one in the current session. Then follow [Shipping a release](docs/development.md#shipping-a-release), which covers picking the version, writing release notes, running `just bump-version X.Y.Z`, and tagging. Never choose a major version unless the maintainer asks for one, and never tag or publish at any other time.

## How to work

- Make the smallest complete change. Reuse existing patterns. No speculative abstractions, unused dependencies, or boilerplate.
- Decide routine, reversible details yourself and state your assumptions. Ask only when missing intent would materially change the work.
- Never claim a platform, feature, or security property works unless you verified it. Report what you verified and what you did not.
- Keep docs in `docs/` updated in the same change as code.

## Verification

- Run `just verify` to execute the full Rust + Flutter verification suite (see `just --list --list-submodules` for all recipes in `justfile` and `just/*.just`).
- **Rust changes:** `just check fmt-rust`, `just check lint`, `just test rust` (`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`).
- **Flutter/Dart changes:** `just check fmt-app`, `just check app`, `just test app` (`dart format --output=none --set-exit-if-changed .`, `flutter analyze`, `flutter test`).
- Check system memory/CPU constraints before heavy builds; never run concurrent Cargo builds on the same target directory.

## Branching, commits, pull requests, and issues

- **Branch workflow:** Open all feature, fix, refactor, docs, and chore pull requests against **`develop`**, never directly against `main`. Start new branches and worktrees from `origin/develop`. `main` is reserved for releases promoted from `develop`.
- Use [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary` (`feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`).
- End every pull request and issue description with one final line naming **all** models that helped with the work (both the coding/review models and any separate model that drafted the PR/issue text), e.g. `Assisted by Claude Haiku 5.5 and Gemini 4 Neon.`
