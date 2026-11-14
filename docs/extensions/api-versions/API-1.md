# API version 1

## Additions

- The client-side extension model, documented in the [extension contract](../README.md)
  and designed in the [extension UI model](../ui-model.md):
  - Manifest fields: `id`, `name`, `description`, `author`, `api_version`, `client`,
    `service`, `capabilities`, `[pane]`, `[settings_page]`, `navigation`, `[bus]` and
    `[[settings]]`.
  - Packages with `manifest.toml`, `client/`, an optional service module and `assets/`.
  - Capabilities `account.read`, `messages.list`, `messages.read`, `messages.label`,
    `messages.organize`, `drafts.list`, `drafts.read`, `drafts.write`, `messages.send`,
    `extensions.bus`, `ui.frame`, `ui.frame.remote`, `ui.commands`, `ui.app_actions`,
    `ui.toasts`, `ui.contribute`, `approvals.notify`, `releases.read`, `ai.text`,
    `ai.decide` and `ai.configure`.
  - Client parts in a Web Worker with the `colecto` global (`onmessage`, `send`, `file`),
    the Wasm client ABI, per-extension credentials, the shell protocol (`start`,
    `account`, `theme`, `settings`, `event`, `host`, `bus.request`, `bus.event`, `reply`,
    `ping`; `render`, `patch`, `call`, `pong`), the component vocabulary and theme tokens.
  - Service parts with the Wasm imports `colecto.action`, `colecto.request` and
    `colecto.emit`, answering bus requests and receiving bus events.
  - Owner settings and the extension bus; bus names may contain `.`.

## Breaking changes

- N/A

## Notes

API 1 was reset when the extension system was rebuilt: the alpha's panel and overlay
system (also called API 1, then 2 and 3, from v0.1.0 to v0.5.4) is gone without
compatibility or migration, and Colecto drops packages built for it on startup.

API 1 is unreleased (`released_in: null`), so its parts that are still being built, such
as the rest of the component vocabulary, toasts, commands and contribution slots, are
added to it rather than to a new version until it ships. The contract says what works
today.
