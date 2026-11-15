# API version 1

## Additions

- The client-side extension model, documented in the [extension contract](../README.md)
  and designed in the [extension UI model](../ui-model.md):
  - Manifest fields: `id`, `name`, `description`, `author`, `api_version`, `client`,
    `service`, `capabilities`, `[pane]`, `[settings_page]`, `navigation`, `[overlay]`,
    `[bus]` and `[[settings]]`.
  - Packages with `manifest.toml`, `client/`, an optional service module and `assets/`.
  - Capabilities `account.read`, `messages.list`, `messages.read`, `messages.label`,
    `messages.organize`, `drafts.list`, `drafts.read`, `drafts.write`, `messages.send`,
    `extensions.bus`, `ui.frame`, `ui.frame.remote`, `ui.commands`, `ui.app_actions`,
    `ui.toasts`, `ui.contribute`, `ui.canvas`, `approvals.notify`, `releases.read`, `ai.text`,
    `ai.decide` and `ai.configure`.
  - Client parts in a Web Worker with the `colecto` global (`onmessage`, `send`, `file`),
    the Wasm client ABI, per-extension credentials, the shell protocol (`start`,
    `account`, `theme`, `settings`, `event`, `host`, `bus.request`, `bus.event`, `reply`,
    `ping`, `navigate`, `back`, `command`, `app_action`, `toast_action`, `command_query`,
    `command_submit`, `slot`, `canvas`, `resize`, `pointer`; `render`, `patch`, `call`,
    `pong`, `state`, `show`, `leave`, `focus`, `dialog`, `toast`, `toast_dismiss`,
    `commands`, `command_search`, `command_hits`, `app_actions`, `contribute`, `download`,
    `frame`), the component vocabulary, contribution slots, a canvas overlay and theme
    tokens.
  - Per-extension key-value storage (`/extensions/{id}/storage/{key}`).
  - Service parts with the Wasm imports `colecto.action`, `colecto.request` and
    `colecto.emit`, answering bus requests and receiving bus events and new-mail
    (`mail`) events; service operations `labels.list`, `storage.get`, `storage.set`,
    `ai.settings` and `ai.decide`; 64 service requests per invocation.
  - Owner settings and the extension bus; bus names may contain `.`.

## Breaking changes

- N/A

## Notes

API 1 was reset when the extension system was rebuilt: the alpha's panel and overlay
system (also called API 1, then 2 and 3, from v0.1.0 to v0.5.4) is gone without
compatibility or migration, and Colecto drops packages built for it on startup.

API 1 shipped in v0.6.0. Any further change to the extension API adds API 2.
